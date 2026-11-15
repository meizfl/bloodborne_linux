mod app;
mod bootstrap;
mod config;
mod formats;
mod i18n;
mod linker;
mod mods;
mod patches;
mod paths;
mod platform;
mod resources;
mod saves;
mod scroll;
mod tk;
mod ui;

fn main() {
    if let Err(error) = start() {
        eprintln!("Bloodborne launcher: {error}");
        std::process::exit(1);
    }
}

fn start() -> Result<(), String> {
    let arguments: Vec<String> = std::env::args().collect();
    if let Some(index) = arguments.iter().position(|a| a == "--inspect-backup") {
        let path = arguments.get(index + 1).ok_or("Missing backup archive")?;
        let archive = saves::inspect_archive(std::path::Path::new(path))?;
        println!(
            "{}",
            serde_json::json!({"save_files":archive.saves,"metadata_files":archive.metadata,"info":archive.info})
        );
        return Ok(());
    }
    if let Some(index) = arguments.iter().position(|a| a == "--compile-patches") {
        let args = &arguments[index + 1..];
        if args.len() < 5 {
            return Err("Usage: --compile-patches ELF XML VERSION OUTPUT PATCH_NAME...".into());
        }
        let elf = formats::Elf::parse(&std::fs::read(&args[0]).map_err(|e| e.to_string())?)?;
        let xml = std::fs::read_to_string(&args[1]).map_err(|e| e.to_string())?;
        let catalogue = patches::parse(&xml)?;
        let writes = patches::compile(&catalogue, &args[4..], &args[2], &elf)?;
        config::atomic_write(std::path::Path::new(&args[3]), &patches::blob(&writes))
            .map_err(|e| e.to_string())?;
        println!(
            "{}",
            serde_json::json!({"writes":writes.len(),"fingerprint":patches::fingerprint(&writes,&elf)?})
        );
        return Ok(());
    }
    if let Some(index) = std::env::args().position(|argument| argument == "--prepare-game") {
        let game = std::path::PathBuf::from(
            std::env::args()
                .nth(index + 1)
                .ok_or("Missing game directory")?,
        );
        let out = std::path::PathBuf::from(
            std::env::args()
                .nth(index + 2)
                .ok_or("Missing output directory")?,
        );
        println!("{}", linker::prepare(&game, &out)?);
        return Ok(());
    }
    if let Some(index) = std::env::args().position(|argument| argument == "--inspect-game") {
        let game = std::path::PathBuf::from(
            std::env::args()
                .nth(index + 1)
                .ok_or("Missing game directory")?,
        );
        let bytes = std::fs::read(game.join("eboot.bin")).map_err(|e| e.to_string())?;
        let elf = formats::Elf::parse(&bytes)?;
        let memory = elf.image()?;
        println!(
            "{}",
            serde_json::json!({"entry":elf.header.entry,"segments":elf.segments.len(),"missing":elf.missing,"memory_size":memory.len(),"image_sha256":elf.image_hash()?})
        );
        return Ok(());
    }
    bootstrap::toolkit()?;
    let tk = tk::Tk::new(&tk::Tk::library_directory())?;
    tk.register("bb::platform", |_| Ok(platform::label()))?;
    if std::env::args().any(|argument| argument == "--check-tk") {
        let platform = tk.call(&["bb::platform"])?;
        let backend = tk.call(&["tk", "windowingsystem"])?;
        // Metacharacters must stay data when sent across the Rust/Tcl boundary.
        let text = "game {path}; [not-a-command] $HOME\nкириллица";
        if tk.call(&["set", "bb_test", text])? != text {
            return Err("Tcl argument round-trip failed".into());
        }
        println!(
            "{}",
            serde_json::json!({"platform": platform, "backend": backend, "tk": tk.call(&["package", "require", "Tk"] )?, "python": false})
        );
        tk.call(&["destroy", "."])?;
        return Ok(());
    }
    if std::env::args().any(|argument| {
        argument == "--preview-ui" || argument == "--check-ui" || argument == "--snapshot-ui"
    }) {
        let paths = paths::Paths::detect()?;
        let resources = resources::extract(&paths.data.join("cache"))?;
        resources::register_fonts(&resources.join("fonts"))?;
        let language = i18n::select(&paths.settings())?;
        let ui = ui::Ui::load(std::rc::Rc::new(tk), &resources, &paths, &language)?;
        if let Some(index) = std::env::args().position(|argument| argument == "--snapshot-ui") {
            let directory = std::path::PathBuf::from(
                std::env::args()
                    .nth(index + 1)
                    .ok_or("Missing snapshot output directory")?,
            );
            std::fs::create_dir_all(&directory).map_err(|e| e.to_string())?;
            let pages: Vec<String> = ui.manifest["pages"]
                .as_object()
                .unwrap()
                .keys()
                .cloned()
                .collect();
            let mut application = app::App::new(ui, paths)?;
            for page in pages {
                application.navigate(&page)?;
                application.ui.tk.call(&["update"])?;
                application.drain()?;
                application.ui.tk.call(&["update"])?;
                application.drain()?;
                application
                    .ui
                    .tk
                    .snapshot(&directory.join(format!("{page}.ppm")))?;
            }
            application.ui.tk.call(&["destroy", "."])?;
            return Ok(());
        }
        if std::env::args().any(|argument| argument == "--check-ui") {
            ui.tk.call(&["update"])?;
            println!(
                "Rust UI: {} pages, {} widgets",
                ui.manifest["pages"].as_object().unwrap().len(),
                ui.manifest["widgets"].as_array().unwrap().len()
            );
            ui.tk.call(&["destroy", "."])?;
            return Ok(());
        }
        let mut application = app::App::new(ui, paths)?;
        application.run()?;
        return Ok(());
    }
    Err("Rust migration is in progress. Use --preview-ui for the development interface.".into())
}
