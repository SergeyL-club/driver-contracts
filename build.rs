use std::env;
use std::fs;
use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-changed=src/lib.rs");

    let crate_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    let profile = env::var("PROFILE").unwrap(); // "debug" или "release"
    let package_name = env::var("CARGO_PKG_NAME").unwrap();

    // 1. Создаем целевую папку target/finally
    let finally_dir = PathBuf::from(&crate_dir).join("target").join("finally");

    fs::create_dir_all(&finally_dir).expect("Не удалось создать директорию target/finally");

    // 2. ГЕНЕРАЦИЯ И ЗАПИСЬ .h ФАЙЛА В target/finally
    let header_file = finally_dir.join(format!("{}.h", package_name));
    cbindgen::generate(&crate_dir)
        .expect("Unable to generate bindings")
        .write_to_file(header_file);

    // 3. ГЕНЕРАЦИЯ И ЗАПИСЬ wrapper.rs В target/finally
    let wrapper_file = finally_dir.join("struct_builder_wrapper.rs");
    let wrapper_code = include_str!("src/wrapper.rs");
    fs::write(wrapper_file, wrapper_code).expect("Не удалось записать wrapper.rs");

    // 4. КОПИРОВАНИЕ ДИНАМИЧЕСКОЙ БИБЛИОТЕКИ В target/finally
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
        println!("cargo:warning=🔥 Все артефакты ABI успешно собраны в папе target/finally!");
    } else {
        // На самом первом проходе сборки бинарника еще нет, предупреждаем пользователя
        println!(
            "cargo:warning=⚠️ Библиотека компилируется. Запустите 'cargo build' повторно для копирования .so/.dll."
        );
    }
}
