use libloading::Library;
use std::path::PathBuf;
use std::sync::Arc;

// Подключаем wrapper.rs, который используют Plugin_A и Plugin_B
#[path = "../src/wrapper.rs"]
mod wrapper;

use wrapper::SafeStructBuilder;
use wrapper::types::{AbiString, TypeBase};

// =========================================================================
// ВСПОМОГАТЕЛЬНЫЕ ФУНКЦИИ ДЛЯ ЗАГРУЗКИ DLL
// =========================================================================

// Универсальный помощник для сборки пути к директории с артефактами
fn get_target_dir() -> PathBuf {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("target");
    path.push("finally"); // Используем папку finally, где собран полный комплект
    path
}

// Путь к вашей реальной DLL контрактов
fn get_contract_dll_path() -> PathBuf {
    let mut path = get_target_dir();
    #[cfg(target_os = "windows")]
    path.push("driver_contracts.dll");
    #[cfg(target_os = "linux")]
    path.push("libdriver_contracts.so");
    #[cfg(target_os = "macos")]
    path.push("libdriver_contracts.dylib");
    path
}

// Помощник для быстрой сборки vtable из DLL
unsafe fn load_core_vtable(lib: &Library) -> Arc<wrapper::CoreVTable> {
    unsafe {
        Arc::new(wrapper::CoreVTable {
            struct_builder_create: *lib
                .get(b"struct_builder_create\0")
                .expect("sym missing: struct_builder_create"),
            struct_builder_add_field: *lib
                .get(b"struct_builder_add_field\0")
                .expect("sym missing: struct_builder_add_field"),
            struct_builder_remove_field: *lib
                .get(b"struct_builder_remove_field\0")
                .expect("sym missing: struct_builder_remove_field"),
            struct_builder_clear_fields: *lib
                .get(b"struct_builder_clear_fields\0")
                .expect("sym missing: struct_builder_clear_fields"),
            struct_builder_destroy: *lib
                .get(b"struct_builder_destroy\0")
                .expect("sym missing: struct_builder_destroy"),
        })
    }
}

// =========================================================================
// ЧАСТЬ 1: БАЗОВОЕ ВЫРАВНИВАНИЕ И ФУНКЦИОНАЛ (Интеграция с DLL)
// =========================================================================

#[test]
fn test_dll_loading_and_basic_interaction() {
    let dll_path = get_contract_dll_path();
    assert!(dll_path.exists(), "DLL не найдена по пути: {:?}", dll_path);

    unsafe {
        // 1. Загружаем DLL
        let core_lib = Library::new(&dll_path).unwrap_or_else(|e| {
            panic!("Не удалось загрузить DLL {:?}: {}", dll_path, e);
        });

        // 2. Инициализируем vtable через наши гарантированные функции
        let vtable = load_core_vtable(&core_lib);

        // 3. Создаем билдер структуры через динамический вызов
        let mut builder = SafeStructBuilder::new("DllBasicTest", vtable).unwrap();

        // Добавляем скалярное поле для проверки выравнивания памяти
        builder.add_field("status", TypeBase::INT32);

        // Проверяем, что лейаут структуры прочитался корректно из памяти
        let (size, align) = builder.get_layout();
        assert_eq!(size, 4, "Размер структуры INT32 должен быть равен 4 байтам");
        assert_eq!(
            align, 4,
            "Выравнивание структуры INT32 должно быть равно 4 байтам"
        );
    }
}

#[test]
fn test_dll_string_passing() {
    let dll_path = get_contract_dll_path();
    assert!(dll_path.exists(), "DLL не найдена по пути: {:?}", dll_path);

    unsafe {
        let core_lib = Library::new(&dll_path).unwrap();
        let vtable = load_core_vtable(&core_lib);

        // Создаем структуру
        let mut builder = SafeStructBuilder::new("StringTestStructure", vtable).unwrap();

        // Проверяем корректность маршалинга и передачи строк по ABI в функцию add_field
        let custom_str = "dynamic_abi_string_field_name";
        let abi_str = AbiString(custom_str.as_ptr(), custom_str.len());

        // Вызов add_field принимает строку по ABI.
        builder.add_field("payload", TypeBase::STRING(abi_str));

        // Если мы дошли до сюда, строка успешно распарсилась на стороне DLL
        let (size, _) = builder.get_layout();
        assert!(
            size > 0,
            "Структура со строковым полем не должна быть пустой"
        );
    }
}

#[test]
fn test_dll_field_removal_flow() {
    let dll_path = get_contract_dll_path();
    assert!(dll_path.exists(), "DLL не найдена по пути: {:?}", dll_path);

    unsafe {
        let core_lib = Library::new(&dll_path).unwrap();
        let vtable = load_core_vtable(&core_lib);

        // 1. Создаем структуру и добавляем два поля
        let mut builder = SafeStructBuilder::new("RemovalTest", vtable).unwrap();
        builder.add_field("first_field", TypeBase::INT32);
        builder.add_field("second_field", TypeBase::INT64);

        let (size_before, _) = builder.get_layout();
        // Предполагаем базовый лейаут: 4 (INT32) + 4 (padding) + 8 (INT64) = 16 байт
        assert_eq!(
            size_before, 16,
            "Исходный размер структуры должен быть 16 байт"
        );

        // 2. Вызываем удаление поля (это задействует метод и поле vtable из варнинга!)
        let removed = builder.remove_field("first_field");
        assert!(
            removed,
            "DLL должна успешно удалить существующее поле 'first_field'"
        );

        // 3. Проверяем, что изменения применились на стороне DLL
        let (size_after, align_after) = builder.get_layout();
        assert_eq!(
            size_after, 8,
            "После удаления INT32 должно остаться только поле INT64 (8 байт)"
        );
        assert_eq!(
            align_after, 8,
            "Выравнивание оставшегося поля должно быть 8"
        );

        // 4. Проверяем поведение при удалении несуществующего поля
        let removed_fake = builder.remove_field("non_existent_field");
        assert!(
            !removed_fake,
            "Удаление несуществующего поля должно вернуть false"
        );
    }
}

// =========================================================================
// ЧАСТЬ 2: СЦЕНАРИЙ МЕЖ-DLL ВЗАИМОДЕЙСТВИЯ (Plugin_A -> CoreDLL -> Plugin_B)
// =========================================================================

#[test]
fn test_multi_dll_ownership_transfer_flow() {
    let dll_path = get_contract_dll_path();
    assert!(dll_path.exists(), "DLL не найдена по пути: {:?}", dll_path);

    unsafe {
        let lib = Library::new(&dll_path).unwrap();
        let shared_vtable = load_core_vtable(&lib);

        // --- ШАГ 1: Логика на стороне Plugin_A ---
        let mut plugin_a_builder =
            SafeStructBuilder::new("SharedCrossDllStructure", shared_vtable.clone()).unwrap();
        plugin_a_builder.add_field("plugin_a_field", TypeBase::INT32);

        // Превращаем в сырой указатель, Drop не вызывается!
        let raw_abi_pointer = plugin_a_builder.into_raw();
        assert!(!raw_abi_pointer.is_null());

        // --- ИМИТАЦИЯ ABI ПЕРЕЛЕТА УКАЗАТЕЛЯ ИЗ PLUGIN_A В PLUGIN_B ---
        let received_pointer_in_plugin_b = raw_abi_pointer;

        // --- ШАГ 2: Логика на стороне Plugin_B ---
        // Восстанавливаем структуру в Plugin_B, передавая ту же vtable
        let mut plugin_b_builder =
            SafeStructBuilder::from_raw(received_pointer_in_plugin_b, shared_vtable.clone());

        let (initial_size, _) = plugin_b_builder.get_layout();
        assert_eq!(initial_size, 4);

        plugin_b_builder.add_field("plugin_b_field", TypeBase::INT64);

        let (final_size, final_align) = plugin_b_builder.get_layout();
        assert_eq!(final_size, 16);
        assert_eq!(final_align, 8);

        // --- ШАГ 3: Финал владения ---
        // Автоматический drop в конце области видимости очистит структуру целиком через DLL деструктор
    }
}

#[test]
fn test_multi_dll_deep_clear_leak_protection() {
    let dll_path = get_contract_dll_path();
    assert!(dll_path.exists(), "DLL не найдена по пути: {:?}", dll_path);

    unsafe {
        let lib = Library::new(&dll_path).unwrap();
        let shared_vtable = load_core_vtable(&lib);

        // Имитируем создание структуры в контексте Plugin_A
        let mut plugin_a_builder =
            SafeStructBuilder::new("StressStructure", shared_vtable.clone()).unwrap();
        let custom_str = "temporary_abi_string_buffer";
        let abi_str = AbiString(custom_str.as_ptr(), custom_str.len());

        plugin_a_builder.add_field("dynamic_string_field", TypeBase::STRING(abi_str));
        plugin_a_builder.add_field("another_field", TypeBase::INT64);

        let raw_ptr = plugin_a_builder.into_raw();

        // Имитируем передачу: Plugin_B забирает владение голым указателем
        let mut plugin_b_builder = SafeStructBuilder::from_raw(raw_ptr, shared_vtable.clone());

        // Очищаем внутренности структуры через плагин B
        plugin_b_builder.clear_fields();

        let (size, align) = plugin_b_builder.get_layout();
        assert_eq!(size, 0);
        assert_eq!(align, 1);

        // Проверяем, что аллокатор работает после очистки и принимает новые поля
        plugin_b_builder.add_field("fresh_start", TypeBase::INT32);

        let (new_size, _) = plugin_b_builder.get_layout();
        assert_eq!(new_size, 4);
    }
}
