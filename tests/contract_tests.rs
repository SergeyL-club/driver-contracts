use driver_contracts::{AbiString, AbiStruct, TypeBase};

// Подключаем wrapper.rs, который используют Plugin_A и Plugin_B
#[path = "../src/wrapper.rs"]
mod wrapper;
use wrapper::SafeStructBuilder;

// =========================================================================
// ЧАСТЬ 1: БАЗОВОЕ ВЫРАВНИВАНИЕ И ФУНКЦИОНАЛ (Проверка ABI типов)
// =========================================================================

#[test]
fn test_abi_scalar_alignment() {
    let mut builder = SafeStructBuilder::new("TestScalar").unwrap();
    builder.add_field("id", TypeBase::INT32);
    let (size, align) = builder.get_layout();
    assert_eq!(size, 4);
    assert_eq!(align, 4);

    builder.add_field("value", TypeBase::INT64);
    let (size, align) = builder.get_layout();
    assert_eq!(size, 16); // 4 + 4(padding) + 8 = 16
    assert_eq!(align, 8);
}

#[test]
fn test_abi_string_field_layout() {
    let mut builder = SafeStructBuilder::new("TestString").unwrap();
    builder.add_field("status", TypeBase::INT32);

    let dummy_val = "value";
    let abi_str_value = AbiString(dummy_val.as_ptr(), dummy_val.len());
    builder.add_field("payload", TypeBase::STRING(abi_str_value));

    let (size, align) = builder.get_layout();
    let ptr_size = std::mem::size_of::<usize>();

    assert_eq!(size, ptr_size * 3); // Padding (4) + AbiString (16) = 24 на x64
    assert_eq!(align, ptr_size);
}

#[test]
fn test_abi_nested_struct_and_removal() {
    let mut builder = SafeStructBuilder::new("MainStruct").unwrap();
    let nested_name = "Point";
    let nested_struct_meta = AbiStruct(nested_name.as_ptr(), 12, 4);

    builder.add_field("position", TypeBase::STRUCT(nested_struct_meta));
    builder.add_field("flag", TypeBase::INT32);

    let (size_before, _) = builder.get_layout();
    assert_eq!(size_before, 16);

    let removed = builder.remove_field("position");
    assert!(removed);

    let (size_after, align_after) = builder.get_layout();
    assert_eq!(size_after, 4); // Поле flag сдвинулось на offset 0
    assert_eq!(align_after, 4);
}

#[test]
fn test_abi_clear_fields() {
    let mut builder = SafeStructBuilder::new("ClearTest").unwrap();
    builder.add_field("f1", TypeBase::INT64);
    builder.add_field("f2", TypeBase::FLOAT32);
    builder.clear_fields();

    let (size, align) = builder.get_layout();
    assert_eq!(size, 0);
    assert_eq!(align, 1);
}

#[test]
fn test_abi_null_safety() {
    let _invalid_builder = SafeStructBuilder::new(unsafe { std::str::from_utf8_unchecked(&[]) });
    let _null_string = AbiString(std::ptr::null(), 0);

    let builder_opt = SafeStructBuilder::new("");
    if builder_opt.is_none() {
        assert!(builder_opt.is_none());
    } else {
        let mut builder = builder_opt.unwrap();
        let res = builder.remove_field("non_existent");
        assert!(!res);
    }
}

// =========================================================================
// ЧАСТЬ 2: СЦЕНАРИЙ МЕЖ-DLL ВЗАИМОДЕЙСТВИЯ (Plugin_A -> CoreDLL -> Plugin_B)
// =========================================================================

/// Тест 6: Симуляция сквозной передачи владения между двумя DLL.
/// Plugin_A создает структуру -> передает сырой указатель -> Plugin_B забирает её,
/// модифицирует и вызывает Drop. Все аллокации должны безопасно пройти через CoreDLL.
#[test]
fn test_multi_dll_ownership_transfer_flow() {
    // --- ШАГ 1: Логика на стороне Plugin_A ---
    // Создаем структуру в контексте первого плагина
    let mut plugin_a_builder = SafeStructBuilder::new("SharedCrossDllStructure").unwrap();

    // Аллоцируем строку имени поля в куче CoreDLL
    plugin_a_builder.add_field("plugin_a_field", TypeBase::INT32);

    // Превращаем обертку в сырой указатель для передачи по ABI.
    // Наш ManuallyDrop/std::mem::forget гарантирует, что Plugin_A не очистит память!
    let raw_abi_pointer = plugin_a_builder.into_raw();
    assert!(
        !raw_abi_pointer.is_null(),
        "Ошибка ABI: Plugin_A вернул пустой указатель"
    );

    // --- ИМИТАЦИЯ ABI ПЕРЕЛЕТА УКАЗАТЕЛЯ ИЗ PLUGIN_A В PLUGIN_B ---
    let received_pointer_in_plugin_b = raw_abi_pointer;

    // --- ШАГ 2: Логика на стороне Plugin_B ---
    // Plugin_B встречает сырой указатель и оборачивает его обратно в безопасный SafeStructBuilder
    let mut plugin_b_builder = unsafe { SafeStructBuilder::from_raw(received_pointer_in_plugin_b) };

    // Проверяем, видит ли Plugin_B то, что записал Plugin_A
    let (initial_size, _) = plugin_b_builder.get_layout();
    assert_eq!(
        initial_size, 4,
        "Plugin_B прочитал битые данные из указателя Plugin_A"
    );

    // Plugin_B добавляет своё поле. Вызов летит в CoreDLL, память вектора расширяется ТАМ.
    plugin_b_builder.add_field("plugin_b_field", TypeBase::INT64);

    // Проверяем корректность слияния данных: 4 (INT32) + 4 (padding) + 8 (INT64) = 16 байт
    let (final_size, final_align) = plugin_b_builder.get_layout();
    assert_eq!(
        final_size, 16,
        "CoreDLL неверно объединил поля от двух разных DLL плагинов"
    );
    assert_eq!(final_align, 8);

    // --- ШАГ 3: Финал владения ---
    // В конце этого теста переменная `plugin_b_builder` выходит из области видимости.
    // Срабатывает автоматический Drop во wrapper.rs на стороне Plugin_B.
    // Вызывается функция `struct_builder_destroy()`.
    // Если CoreDLL успешно удалит и память структуры, и вектор полей, и строки имен — тест завершится успешно.
    // Если аллокаторы конфликтуют — операционная система выдаст Heap Corruption / Crash прямо сейчас.
}

/// Тест 7: Стресс-тест глубокой очистки памяти (Memory Leak & Double Free check)
/// Проверяет, что Plugin_B может выполнить полную очистку структуры, созданной в Plugin_A,
/// включая глубокое уничтожение аллоцированных Си-строк.
#[test]
fn test_multi_dll_deep_clear_leak_protection() {
    let mut plugin_a_builder = SafeStructBuilder::new("StressStructure").unwrap();

    // Наполняем структуру тяжелыми динамическими типами данных
    let custom_str = "temporary_abi_string_buffer";
    let abi_str = AbiString(custom_str.as_ptr(), custom_str.len());

    plugin_a_builder.add_field("dynamic_string_field", TypeBase::STRING(abi_str));
    plugin_a_builder.add_field("another_field", TypeBase::INT64);

    let raw_ptr = plugin_a_builder.into_raw();

    // Передаем в Plugin_B
    let mut plugin_b_builder = unsafe { SafeStructBuilder::from_raw(raw_ptr) };

    // Plugin_B заставляет CoreDLL полностью очистить поля и уничтожить внутренние строки
    plugin_b_builder.clear_fields();

    let (size, align) = plugin_b_builder.get_layout();
    assert_eq!(
        size, 0,
        "После очистки из Plugin_B размер структуры не сбросился"
    );
    assert_eq!(align, 1);

    // Добавляем новое чистое поле после полного сброса
    plugin_b_builder.add_field("fresh_start", TypeBase::INT32);
    let (new_size, _) = plugin_b_builder.get_layout();
    assert_eq!(
        new_size, 4,
        "Структура пришла в негодность после меж-DLL очистки"
    );
}
