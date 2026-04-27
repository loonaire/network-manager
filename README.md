# 🌐 Network Manager — Gestionnaire Proxy & Partages Réseau Windows

> **Prévention: Cet outil a été généré par Claude Code Sonnet 4.6**


Interface graphique native Windows pour :
- **Activer / désactiver le proxy** système (registre Windows) via un interrupteur cliquable
- **Monter / démonter des lecteurs réseau** (UNC paths, ex : `\\serveur\partage`)
- **Afficher les adresses IP** de toutes les interfaces réseau locales

---

## 📋 Prérequis

| Outil         | Version minimale | Téléchargement                          |
|---------------|------------------|-----------------------------------------|
| Rust + Cargo  | 1.75+            | https://rustup.rs                       |
| Windows       | 10 / 11          | —                                       |

> **Droits administrateur recommandés** pour modifier le proxy système et monter des lecteurs réseau.

---

## 🚀 Compilation et exécution

```bat
:: Clonez / copiez le projet, puis dans le dossier :

:: Mode développement (avec fenêtre console)
cargo run

:: Mode release — exécutable optimisé SANS console
cargo build --release

:: L'exécutable se trouve dans :
target\release\network-manager.exe
```

---

## 🖥 Fonctionnalités

### Adresses IP
- Affiche toutes les interfaces réseau IPv4 actives (hors loopback)
- Bouton **⟳ Actualiser** pour mettre à jour en temps réel

### Proxy système
- Interrupteur ON/OFF cliquable (rouge = activé, vert = désactivé)
- Écrit directement dans `HKCU\Software\Microsoft\Windows\CurrentVersion\Internet Settings\ProxyEnable`
- Pris en compte par IE, Edge, et tous les logiciels utilisant WinInet

### Partages réseau
| Colonne      | Description                              |
|--------------|------------------------------------------|
| Lettre       | Lettre du lecteur (ex : `Z:`, `Y:`)      |
| Chemin réseau| Partage UNC (ex : `\\serveur\partage`)     |
| Utilisateur  | Optionnel — pour l'authentification      |
| Mot de passe | Optionnel — masqué dans l'interface      |

- **🔗 Connecter** : exécute `net use U: \\serveur\partage /persistent:yes`
- **⏏ Déconnecter** : exécute `net use U: /delete`
- **🗑 Supprimer** : retire la ligne du tableau
- **➕ Ajouter** : crée une nouvelle ligne

---

## 📦 Dépendances (Cargo.toml)

```toml
eframe   = "0.28"   # framework GUI (egui)
egui     = "0.28"   # interface graphique immédiate
winreg   = "0.52"   # accès au registre Windows
if-addrs = "0.13"   # énumération des interfaces réseau
```

---

## 🗂 Structure du projet

```
network-manager/
├-- Cargo.toml
├-- README.md
└-- src/
    └-- main.rs      # tout le code source (~350 lignes)
```

---

## ⚙️ Personnalisation

Modifiez les lecteurs pré-remplis dans `App::new()` (src/main.rs) :

```rust
drives: vec![
    Drive::new("Z:", r"\\serveur\partage1"),
    Drive::new("Y:", r"\\serveur\partage2"),
    Drive::new("W:", r"\\nas\backup"),   // ← ajoutez autant que nécessaire
],
```

---

## 📝 Licence

MIT — libre d'utilisation et de modification.
