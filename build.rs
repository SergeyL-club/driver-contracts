use std::env;
use std::fs;
use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-changed=");

    let crate_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    let package_name = env::var("CARGO_PKG_NAME").unwrap();

    // Определяем профиль сборки (debug или release) максимально надёжным способом
    let profile = env::var("CARGO_BUILD_PROFILE")
        .or_else(|_| env::var("PROFILE"))
        .unwrap_or_else(|_| "debug".to_string());

    // Говорим Cargo перезапускать build.rs, если изменился любой исходник или types/wrapper
    println!("cargo:rerun-if-changed=src/");

    // 1. Создаем целевую папку target/finally
    let finally_dir = PathBuf::from(&crate_dir).join("target").join("finally");
    fs::create_dir_all(&finally_dir).expect("Не удалось создать директорию target/finally");

    // 2. ГЕНЕРАЦИЯ И ЗАПИСЬ .h ФАЙЛА В target/finally
    let header_file = finally_dir.join(format!("{}.h", package_name));
    cbindgen::generate(&crate_dir)
        .expect("Unable to generate bindings")
        .write_to_file(header_file);

    // Ссылка на папку src для динамического чтения файлов
    let src_dir = PathBuf::from(&crate_dir).join("src");

    // 3. КОПИРОВАНИЕ wrapper.rs В target/finally
    let wrapper_dest = finally_dir.join("struct_builder_wrapper.rs");
    let wrapper_code = fs::read_to_string(src_dir.join("wrapper.rs"))
        .expect("Не удалось прочитать src/wrapper.rs с диска");
    fs::write(wrapper_dest, wrapper_code).expect("Не удалось записать wrapper.rs");

    // 4. КОПИРОВАНИЕ types.rs В target/finally
    let types_dest = finally_dir.join("types-contract.rs");
    let types_code = fs::read_to_string(src_dir.join("types.rs"))
        .expect("Не удалось прочитать src/types.rs с диска");
    fs::write(types_dest, types_code).expect("Не удалось записать types.rs");

    // 5. КОПИРОВАНИЕ ДИНАМИЧЕСКОЙ БИБЛИОТЕКИ В target/finally (С поддержкой Debug/Release)
    let target_dir = PathBuf::from(&crate_dir).join("target").join(&profile);

    // Определяем имя бинарника под текущую ОС
    let (prefix, extension) = if cfg!(target_os = "windows") {
        ("", "dll")
    } else if cfg!(target_os = "macos") {
        ("lib", "dylib")
    } else {
        ("lib", "so") // Linux
    };

    let binary_name = format!("{}{}.{}", prefix, package_name.replace("-", "_"), extension);
    let src_binary = target_dir.join(&binary_name);
    let dest_binary = finally_dir.join(&binary_name);

    // Копируем бинарник, если Cargo его уже собрал
    if src_binary.exists() {
        fs::copy(&src_binary, &dest_binary)
            .expect("Не удалось скопировать файл динамической библиотеки в target/finally");
        println!(
            "cargo:warning=🔥 Все артефакты ABI успешно собраны в папке target/finally! Профиль: [{}]",
            profile.to_uppercase()
        );
    } else {
        // Выводим подсказку с учётом текущего профиля сборки, чтобы пользователь знал, какую команду повторить
        println!(
            "cargo:warning=⚠️ Библиотека [{}] ещё компилируется. Запустите 'cargo build{}' повторно для копирования бинарника.",
            profile.to_uppercase(),
            if profile == "release" {
                " --release"
            } else {
                ""
            }
        );
    }
}
