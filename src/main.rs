// ============================================================================
//  Network Manager  —  Gestionnaire Proxy & Partages Réseau  (Windows)
//  Compilation : cargo build --release
//  Prérequis   : Windows, droits Admin recommandés pour le proxy
// ============================================================================

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use eframe::egui::{self, Color32, Margin, RichText, Rounding, Vec2};
use std::env;
use std::path::Path;
use std::process::Command;
use winreg::enums::{HKEY_CURRENT_USER, KEY_WRITE};
use winreg::RegKey;

// --- Palette -----------------------------------------------------------------

const GREEN:    Color32 = Color32::from_rgb(52,  199,  89);
const RED:      Color32 = Color32::from_rgb(255,  69,  58);
const ORANGE:   Color32 = Color32::from_rgb(255, 159,  10);
const BLUE:     Color32 = Color32::from_rgb(10,  132, 255);
const HEADER_D: Color32 = Color32::from_gray(50);
const HEADER_L: Color32 = Color32::from_gray(210);

// --- Proxy (registre Windows) ------------------------------------------------

const PROXY_REGKEY: &str =
    r"Software\Microsoft\Windows\CurrentVersion\Internet Settings";

fn proxy_is_enabled() -> bool {
    RegKey::predef(HKEY_CURRENT_USER)
        .open_subkey(PROXY_REGKEY)
        .and_then(|k| k.get_value::<u32, _>("ProxyEnable"))
        .map(|v: u32| v != 0)
        .unwrap_or(false)
}

fn set_proxy_enabled(enabled: bool) -> std::io::Result<()> {
    let key = RegKey::predef(HKEY_CURRENT_USER)
        .open_subkey_with_flags(PROXY_REGKEY, KEY_WRITE)?;
    key.set_value("ProxyEnable", &(enabled as u32))?;
    Ok(())
}

// --- Adresses IP -------------------------------------------------------------

fn get_ip_list() -> Vec<(String, String)> {
    if_addrs::get_if_addrs()
        .unwrap_or_default()
        .into_iter()
        .filter(|i| !i.is_loopback() && i.addr.ip().is_ipv4())
        .map(|i| (i.name.clone(), i.addr.ip().to_string()))
        .collect()
}

// --- Lecteurs réseau ----------------------------------------------------------

/// Vérifie si un lecteur est monté en testant l'existence du chemin `X:\`.
fn drive_is_mounted(letter: &str) -> bool {
    let l = letter.trim_end_matches(':').trim();
    let root = format!("{}:\\", l);
    Path::new(&root).exists()
}

fn net_use_connect(letter: &str, path: &str, user: &str, pass: &str) -> Result<(), String> {
    let drive = format!("{}:", letter.trim_end_matches(':').trim());
    let mut cmd = Command::new("net");
    cmd.args(["use", &drive, path, "/persistent:yes"]);
    if !user.is_empty() {
        cmd.arg(format!("/user:{}", user));
        if !pass.is_empty() {
            cmd.arg(pass);
        }
    }
    let out = cmd.output().map_err(|e| e.to_string())?;
    if out.status.success() {
        Ok(())
    } else {
        let msg = format!(
            "{}{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
        Err(msg.trim().to_string())
    }
}

fn net_use_disconnect(letter: &str) -> Result<(), String> {
    let drive = format!("{}:", letter.trim_end_matches(':').trim());
    let out = Command::new("net")
        .args(["use", &drive, "/delete", "/y"])
        .output()
        .map_err(|e| e.to_string())?;
    if out.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&out.stderr).trim().to_string())
    }
}

// --- Modèle ------------------------------------------------------------------

/// Un lecteur réseau défini en dur — non modifiable par l'utilisateur.
#[derive(Clone)]
struct Drive {
    letter:  String,  // ex : "U:"
    path:    String,  // ex : r"\\serveur\users"
    label:   String,  // description lisible
    user:    String,  // identifiant optionnel
    pass:    String,  // mot de passe optionnel
    mounted: bool,    // statut mis à jour par refresh_status()
}

impl Drive {
    fn new(letter: &str, path: &str, label: &str) -> Self {
        Self {
            letter:  letter.into(),
            path:    path.into(),
            label:   label.into(),
            user:    String::new(),
            pass:    String::new(),
            mounted: false,
        }
    }

    #[allow(dead_code)]
    fn new_auth(letter: &str, path: &str, label: &str, user: &str, pass: &str) -> Self {
        Self { user: user.into(), pass: pass.into(), ..Self::new(letter, path, label) }
    }

    fn refresh_status(&mut self) {
        self.mounted = drive_is_mounted(&self.letter);
    }
}

// --- Événements --------------------------------------------------------------

enum Ev {
    ToggleProxy,
    RefreshIps,
    RefreshDrives,
    Connect(usize),
    Disconnect(usize),
    ConnectAll,
}

// --- Application -------------------------------------------------------------

struct App {
    proxy_on:       bool,
    ips:            Vec<(String, String)>,
    drives:         Vec<Drive>,
    msg:            Option<(String, bool)>,
    last_refresh_s: f64,
}

impl App {
    fn new() -> Self {
        // ====================================================================
        //  ► CONFIGUREZ ICI VOS LECTEURS RÉSEAU ◄
        //  Drive::new(lettre, chemin_unc, description)
        //  Drive::new_auth(lettre, chemin, description, utilisateur, mot_de_passe)
        // ====================================================================
        let mut drives = vec![
            Drive::new("Z:", r"\\serveur\share1",  "Dossiers partage 1"),
            Drive::new("Y:", r"\\serveur\share2",  "Données partage 2"),
            Drive::new("W:", r"\\serveur\backup", "Sauvegardes"),
        ];
        for d in &mut drives {
            d.refresh_status();
        }
        Self {
            proxy_on:       proxy_is_enabled(),
            ips:            get_ip_list(),
            drives,
            msg:            None,
            last_refresh_s: 0.0,
        }
    }

    fn handle(&mut self, ev: Ev) {
        match ev {
            Ev::ToggleProxy => {
                let new = !self.proxy_on;
                match set_proxy_enabled(new) {
                    Ok(()) => {
                        self.proxy_on = new;
                        self.msg = Some((
                            format!("Proxy {}", if new { "activé" } else { "désactivé" }),
                            true,
                        ));
                    }
                    Err(e) => self.msg = Some((format!("Erreur proxy : {}", e), false)),
                }
            }
            Ev::RefreshIps => {
                self.ips = get_ip_list();
                self.msg = Some(("Adresses IP actualisées".into(), true));
            }
            Ev::RefreshDrives => {
                for d in &mut self.drives {
                    d.refresh_status();
                }
            }
            Ev::Connect(i) => {
                let d = self.drives[i].clone();
                match net_use_connect(&d.letter, &d.path, &d.user, &d.pass) {
                    Ok(()) => {
                        self.drives[i].refresh_status();
                        self.msg = Some((
                            format!("Lecteur {} connecté ({}).", d.letter, d.path),
                            true,
                        ));
                    }
                    Err(e) => {
                        self.drives[i].refresh_status();
                        self.msg = Some((
                            format!("Erreur connexion {} : {}", d.letter, e),
                            false,
                        ));
                    }
                }
            }
            Ev::Disconnect(i) => {
                let l = self.drives[i].letter.clone();
                match net_use_disconnect(&l) {
                    Ok(()) => {
                        self.drives[i].refresh_status();
                        self.msg = Some((format!("Lecteur {} déconnecté.", l), true));
                    }
                    Err(e) => {
                        self.drives[i].refresh_status();
                        self.msg = Some((format!("Erreur déconnexion {} : {}", l, e), false));
                    }
                }
            }
            Ev::ConnectAll => {
                let mut errors = vec![];
                for i in 0..self.drives.len() {
                    if !self.drives[i].mounted {
                        let d = self.drives[i].clone();
                        if let Err(e) = net_use_connect(&d.letter, &d.path, &d.user, &d.pass) {
                            errors.push(format!("{} : {}", d.letter, e));
                        }
                        self.drives[i].refresh_status();
                    }
                }
                self.msg = if errors.is_empty() {
                    Some(("Tous les lecteurs sont connectés.".into(), true))
                } else {
                    Some((format!("Erreurs : {}", errors.join(" | ")), false))
                };
            }
        }
    }
}

// --- Widget : toggle switch ---------------------------------------------------

fn toggle_switch(ui: &mut egui::Ui, on: bool) -> bool {
    let w = 52.0_f32;
    let h = 26.0_f32;
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, h), egui::Sense::click());
    if ui.is_rect_visible(rect) {
        let bg    = if on { RED } else { GREEN };
        let t     = if on { 1.0_f32 } else { 0.0_f32 };
        let label = if on { "ON" } else { "OFF" };
        let font  = egui::FontId::proportional(9.0);
        ui.painter().rect_filled(rect, Rounding::same(h / 2.0), bg);
        let tpos = if on {
            egui::pos2(rect.left() + 5.0, rect.center().y)
        } else {
            egui::pos2(rect.right() - 18.0, rect.center().y)
        };
        ui.painter().text(tpos, egui::Align2::LEFT_CENTER, label, font, Color32::WHITE);
        let kx = rect.left() + h / 2.0 + t * (w - h);
        ui.painter().circle_filled(egui::pos2(kx, rect.center().y), h / 2.0 - 2.0, Color32::WHITE);
        ui.painter().circle_stroke(
            egui::pos2(kx, rect.center().y),
            h / 2.0 - 2.0,
            egui::Stroke::new(1.0, Color32::from_black_alpha(40)),
        );
    }
    resp.clicked()
}

// --- Widget : voyant de statut -----------------------------------------------

fn status_indicator(ui: &mut egui::Ui, mounted: bool) {
    let (color, label) = if mounted {
        (GREEN, "Connecté   ")
    } else {
        (Color32::from_gray(115), "Déconnecté")
    };
    let (rect, _) = ui.allocate_exact_size(Vec2::new(106.0, 26.0), egui::Sense::hover());
    if ui.is_rect_visible(rect) {
        let cx = rect.left() + 9.0;
        let cy = rect.center().y;
        // Halo pulsant (toujours visible si connecté)
        if mounted {
            ui.painter().circle_filled(
                egui::pos2(cx, cy),
                9.0,
                GREEN.linear_multiply(0.18),
            );
        }
        // Pastille principale
        ui.painter().circle_filled(egui::pos2(cx, cy), 6.0, color);
        // Reflet intérieur clair
        ui.painter().circle_filled(
            egui::pos2(cx - 1.5, cy - 1.5),
            2.0,
            Color32::from_white_alpha(90),
        );
        // Bordure fine
        ui.painter().circle_stroke(
            egui::pos2(cx, cy),
            6.0,
            egui::Stroke::new(1.0, Color32::from_black_alpha(50)),
        );
        // Texte
        ui.painter().text(
            egui::pos2(cx + 14.0, cy),
            egui::Align2::LEFT_CENTER,
            label,
            egui::FontId::proportional(12.0),
            color,
        );
    }
}

// --- Interface principale -----------------------------------------------------

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let mut events: Vec<Ev> = Vec::new();
        let dark = ctx.style().visuals.dark_mode;

        // Rafraîchissement automatique toutes les 5 s
        let now = ctx.input(|i| i.time);
        if now - self.last_refresh_s > 5.0 {
            self.last_refresh_s = now;
            events.push(Ev::RefreshDrives);
        }

        egui::CentralPanel::default().show(ctx, |ui| {

            // == En-tête ======================================================
            ui.add_space(10.0);
            ui.horizontal(|ui| {
                ui.heading(RichText::new("🌐  Network Manager").size(24.0).strong());
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(RichText::new("Windows Network Tool  v1.1").strong().weak().italics());
                });
            });
            ui.separator();
            ui.add_space(8.0);

            // == Bloc Nom d'utilisateur =======================================
            frame_card(ui, dark, |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Nom d'utilisateur: ").strong().weak());
                    ui.label(RichText::new(env::var("USERNAME").unwrap_or("Inconnu".to_string())).strong().weak());
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {   
                        ui.add_space(1.0);                
                    });
                });

            });

            // == Bloc Adresses IP =============================================
            frame_card(ui, dark, |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new("🖥  Adresses IP locales").strong().size(13.0));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.add(egui::Button::new("⟳  Actualiser").small())
                            .on_hover_text("Rafraîchir la liste des interfaces")
                            .clicked()
                        {
                            events.push(Ev::RefreshIps);
                        }
                    });
                });
                ui.add_space(6.0);
                ui.horizontal_wrapped(|ui| {
                    if self.ips.is_empty() {
                        ui.label(RichText::new("Aucune interface réseau détectée").italics().weak());
                    }
                    for (name, ip) in &self.ips {
                        frame_pill(ui, dark, |ui| {
                            ui.vertical_centered(|ui| {
                                ui.label(RichText::new(name).strong().weak());
                                ui.label(RichText::new(ip).monospace().size(14.0).color(BLUE));
                            });
                        });
                        ui.add_space(4.0);
                    }
                });
            });

            ui.add_space(10.0);

            // == Bloc Proxy ===================================================
            let proxy_fill   = proxy_bg(self.proxy_on, dark);
            let proxy_stroke = if self.proxy_on {
                Color32::from_rgb(180, 60, 60)
            } else {
                Color32::from_rgb(52, 160, 70)
            };
            egui::Frame {
                fill:         proxy_fill,
                inner_margin: Margin::symmetric(14.0, 12.0),
                rounding:     Rounding::same(8.0),
                stroke:       egui::Stroke::new(1.5, proxy_stroke),
                ..Default::default()
            }
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    if toggle_switch(ui, self.proxy_on) {
                        events.push(Ev::ToggleProxy);
                    }
                    ui.add_space(12.0);
                    ui.vertical(|ui| {
                        ui.label(RichText::new("Proxy système").strong().size(14.0));
                        ui.add_space(2.0);
                        let (txt, col) = if self.proxy_on {
                            ("● ACTIVÉ  — Le trafic passe par le proxy", RED)
                        } else {
                            ("● DÉSACTIVÉ  — Connexion directe", GREEN)
                        };
                        ui.label(RichText::new(txt).color(col).size(12.0));
                    });
                });
            });

            ui.add_space(12.0);

            // == Bloc Lecteurs Réseau ==========================================
            frame_card(ui, dark, |ui| {

                // Titre + bouton actualiser
                ui.horizontal(|ui| {
                    ui.label(RichText::new("📁  Lecteurs réseau").strong().size(13.0));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.add(egui::Button::new("⟳  Actualiser les statuts").small())
                            .on_hover_text("Vérifier l'état de tous les lecteurs")
                            .clicked()
                        {
                            events.push(Ev::RefreshDrives);
                        }
                        if ui.add(
                            egui::Button::new(
                                RichText::new("🔗  Tout connecter").size(12.0).color(Color32::WHITE),
                            )
                            .fill(Color32::from_rgb(24, 105, 200)),
                        )
                        .on_hover_text("Monter tous les lecteurs non connectés")
                        .clicked()
                        {
                            events.push(Ev::ConnectAll);
                        }
                    });
                });
                ui.add_space(6.0);

                // En-tête tableau
                egui::Frame {
                    fill:         if dark { HEADER_D } else { HEADER_L },
                    inner_margin: Margin::symmetric(10.0, 5.0),
                    rounding:     Rounding::same(4.0),
                    ..Default::default()
                }
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        col_hdr(ui, "Statut",      106.0);
                        col_hdr(ui, "Lettre",       52.0);
                        col_hdr(ui, "Description", 158.0);
                        col_hdr(ui, "Chemin UNC",  218.0);
                        col_hdr(ui, "Action",      100.0);
                    });
                });

                // Lignes
                egui::ScrollArea::vertical()
                    .max_height(220.0)
                    .auto_shrink([false, true])
                    .id_source("drives_scroll")
                    .show(ui, |ui| {
                        for i in 0..self.drives.len() {
                            let mounted = self.drives[i].mounted;

                            egui::Frame {
                                fill: row_bg(i, mounted, dark),
                                inner_margin: Margin::symmetric(10.0, 5.0),
                                rounding:     Rounding::same(4.0),
                                stroke: egui::Stroke::new(
                                    1.0,
                                    if mounted {
                                        GREEN.linear_multiply(0.25)
                                    } else {
                                        Color32::TRANSPARENT
                                    },
                                ),
                                ..Default::default()
                            }
                            .show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    // Voyant statut
                                    status_indicator(ui, mounted);

                                    // Lettre
                                    ui.add_sized([52.0, 22.0], egui::Label::new(
                                        RichText::new(&self.drives[i].letter)
                                            .monospace().size(14.0).strong().color(ORANGE),
                                    ));

                                    // Description
                                    ui.add_sized([158.0, 22.0], egui::Label::new(
                                        RichText::new(&self.drives[i].label).size(12.0),
                                    ));

                                    // Chemin UNC
                                    ui.add_sized([218.0, 22.0], egui::Label::new(
                                        RichText::new(&self.drives[i].path)
                                            .monospace().size(11.0).weak(),
                                    ));

                                    // -- Bouton unique contextuel ----------
                                    if mounted {
                                        // Lecteur monté → Déconnecter
                                        if ui.add(
                                            egui::Button::new(
                                                RichText::new("⏏  Déconnecter")
                                                    .size(12.0).color(Color32::WHITE),
                                            )
                                            .fill(Color32::from_rgb(150, 35, 35))
                                            .min_size(Vec2::new(100.0, 26.0)),
                                        )
                                        .on_hover_text(format!(
                                            "Démonter le lecteur {} (net use /delete)",
                                            self.drives[i].letter
                                        ))
                                        .clicked()
                                        {
                                            events.push(Ev::Disconnect(i));
                                        }
                                    } else {
                                        // Lecteur absent → Créer
                                        if ui.add(
                                            egui::Button::new(
                                                RichText::new("🔗  Créer")
                                                    .size(12.0).color(Color32::WHITE),
                                            )
                                            .fill(Color32::from_rgb(24, 105, 200))
                                            .min_size(Vec2::new(100.0, 26.0)),
                                        )
                                        .on_hover_text(format!(
                                            "Monter {} → {} (net use /persistent:yes)",
                                            self.drives[i].letter, self.drives[i].path
                                        ))
                                        .clicked()
                                        {
                                            events.push(Ev::Connect(i));
                                        }
                                    }
                                });
                            });
                            ui.add_space(2.0);
                        }
                    });

                // Légende
                ui.add_space(4.0);
                ui.separator();
                ui.add_space(2.0);
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Légende : ").strong().weak());
                    legend_dot(ui, GREEN,                   "Lecteur connecté et accessible");
                    ui.add_space(14.0);
                    legend_dot(ui, Color32::from_gray(115), "Lecteur non monté");
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(
                            RichText::new("Mise à jour automatique toutes les 5 s")
                                .weak().italics(),
                        );
                    });
                });
            });

            ui.add_space(8.0);
            ui.separator();
            ui.add_space(4.0);

            // -- Barre de statut --------------------------------------------
            match &self.msg {
                Some((text, true)) => {
                    ui.label(RichText::new(format!("✅  {}", text)).color(GREEN).size(13.0));
                }
                Some((text, false)) => {
                    egui::ScrollArea::vertical()
                        .max_height(50.0)
                        .id_source("err_scroll")
                        .show(ui, |ui| {
                            ui.label(RichText::new(format!("❌  {}", text)).color(RED).size(13.0));
                        });
                }
                None => {
                    ui.label(RichText::new("Prêt.").weak().small().italics());
                }
            }

            // Repaint toutes les 2 s pour l'auto-refresh
            ctx.request_repaint_after(std::time::Duration::from_secs(2));
        });

        for ev in events {
            self.handle(ev);
        }
    }
}

// --- Helpers UI --------------------------------------------------------------

fn frame_card(ui: &mut egui::Ui, dark: bool, add: impl FnOnce(&mut egui::Ui)) {
    egui::Frame {
        fill:         if dark { Color32::from_gray(30) } else { Color32::from_gray(248) },
        inner_margin: Margin::same(10.0),
        rounding:     Rounding::same(8.0),
        stroke:       egui::Stroke::new(
            1.0,
            if dark { Color32::from_gray(55) } else { Color32::from_gray(218) },
        ),
        ..Default::default()
    }
    .show(ui, add);
}

fn frame_pill(ui: &mut egui::Ui, dark: bool, add: impl FnOnce(&mut egui::Ui)) {
    egui::Frame {
        fill:         if dark { Color32::from_gray(44) } else { Color32::WHITE },
        inner_margin: Margin::symmetric(10.0, 6.0),
        rounding:     Rounding::same(6.0),
        stroke:       egui::Stroke::new(
            1.0,
            if dark { Color32::from_gray(65) } else { Color32::from_gray(200) },
        ),
        ..Default::default()
    }
    .show(ui, add);
}

fn proxy_bg(on: bool, dark: bool) -> Color32 {
    match (on, dark) {
        (true,  true)  => Color32::from_rgb(50, 22, 22),
        (false, true)  => Color32::from_rgb(18, 40, 22),
        (true,  false) => Color32::from_rgb(255, 238, 238),
        (false, false) => Color32::from_rgb(238, 255, 242),
    }
}

fn row_bg(idx: usize, mounted: bool, dark: bool) -> Color32 {
    if mounted {
        if dark { Color32::from_rgb(16, 34, 16) } else { Color32::from_rgb(234, 254, 238) }
    } else if idx % 2 == 0 {
        if dark { Color32::from_gray(26) } else { Color32::WHITE }
    } else {
        if dark { Color32::from_gray(32) } else { Color32::from_gray(251) }
    }
}

fn col_hdr(ui: &mut egui::Ui, text: &str, width: f32) {
    ui.add_sized([width, 18.0], egui::Label::new(RichText::new(text).strong().small()));
}

fn legend_dot(ui: &mut egui::Ui, color: Color32, label: &str) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(12.0, 14.0), egui::Sense::hover());
    ui.painter().circle_filled(rect.center(), 4.5, color);
    ui.label(RichText::new(label).strong().weak());
}

// --- main --------------------------------------------------------------------

fn main() -> Result<(), eframe::Error> {
    let opts = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Network Manager")
            .with_inner_size([760.0, 560.0])
            .with_min_inner_size([640.0, 440.0]),
        ..Default::default()
    };

    eframe::run_native(
        "Network Manager",
        opts,
        Box::new(|cc| {
            let mut visuals          = egui::Visuals::dark();
            visuals.panel_fill       = Color32::from_gray(22);
            visuals.window_fill      = Color32::from_gray(22);
            visuals.extreme_bg_color = Color32::from_gray(14);
            cc.egui_ctx.set_visuals(visuals);
            Ok(Box::new(App::new()))
        }),
    )
}
