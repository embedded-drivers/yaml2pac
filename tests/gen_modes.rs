use std::path::PathBuf;
use tempfile::TempDir;
use yaml2pac::{GenOptions, Mode};

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

fn gen_to_string(yaml: &str, mode: Mode, builtin_common: bool) -> String {
    let ir = yaml2pac::read_ir(fixture(yaml)).unwrap();
    let tmp = TempDir::new().unwrap();
    let out = tmp.path().join("out.rs");
    let opts = GenOptions {
        mode,
        builtin_common,
        common_module_path: None,
    };
    yaml2pac::gen_pac(ir, &out, &opts).unwrap();
    std::fs::read_to_string(&out).unwrap()
}

// --- PAC mode tests ---

#[test]
fn pac_generates_output() {
    let code = gen_to_string("pac_timer.yaml", Mode::Pac, true);
    assert!(!code.is_empty());
}

#[test]
fn pac_contains_register_types() {
    let code = gen_to_string("pac_timer.yaml", Mode::Pac, true);
    // Should contain the timer block and register access
    assert!(code.contains("Reg"), "PAC output should contain Reg type");
}

#[test]
fn pac_contains_fieldset() {
    let code = gen_to_string("pac_timer.yaml", Mode::Pac, true);
    assert!(code.contains("en"), "PAC output should contain 'en' field accessor");
}

#[test]
fn pac_contains_enum() {
    let code = gen_to_string("pac_timer.yaml", Mode::Pac, true);
    assert!(code.contains("OneShot") || code.contains("ONE_SHOT"),
        "PAC output should contain MODE enum variants");
}

#[test]
fn pac_contains_register_array() {
    let code = gen_to_string("pac_timer.yaml", Mode::Pac, true);
    assert!(code.contains("CMP") || code.contains("cmp"),
        "PAC output should contain CMP register array");
}

// --- RV-CSR mode tests ---

#[test]
fn rvcsr_generates_output() {
    let code = gen_to_string("csr_minimal.yaml", Mode::RvCsr, true);
    assert!(!code.is_empty());
}

#[test]
fn rvcsr_generates_csr_modules() {
    let code = gen_to_string("csr_minimal.yaml", Mode::RvCsr, true);
    assert!(code.contains("pub mod mstatus"), "should generate mstatus module");
    assert!(code.contains("pub mod mie"), "should generate mie module");
    assert!(code.contains("pub mod mcause"), "should generate mcause module");
}

#[test]
fn rvcsr_contains_inline_asm() {
    let code = gen_to_string("csr_minimal.yaml", Mode::RvCsr, true);
    // mstatus = 0x300
    assert!(code.contains("csrrs"), "should contain csrrs instruction");
    assert!(code.contains("csrrw"), "should contain csrrw instruction");
    assert!(code.contains("csrrc"), "should contain csrrc instruction");
    assert!(code.contains("0x300"), "should contain mstatus address 0x300");
}

#[test]
fn rvcsr_contains_read_write_functions() {
    let code = gen_to_string("csr_minimal.yaml", Mode::RvCsr, true);
    assert!(code.contains("pub fn read"), "should have read() function");
    assert!(code.contains("pub unsafe fn write"), "should have write() function");
    assert!(code.contains("pub unsafe fn modify"), "should have modify() function");
}

#[test]
fn rvcsr_readonly_no_write() {
    let code = gen_to_string("csr_minimal.yaml", Mode::RvCsr, true);
    // mcause is read-only: find its module and verify no write/set/clear inside
    // Split at module boundaries to isolate mcause module content
    let mcause_start = code.find("pub mod mcause").expect("mcause module should exist");
    // Find the next top-level module after mcause (or end of string)
    let rest = &code[mcause_start..];
    let mcause_end = rest[1..].find("pub mod ").map(|i| i + 1).unwrap_or(rest.len());
    let mcause_code = &rest[..mcause_end];

    assert!(mcause_code.contains("pub fn read"), "read-only CSR should have read()");
    assert!(!mcause_code.contains("pub unsafe fn write"),
        "read-only CSR should NOT have write()");
    assert!(!mcause_code.contains("pub unsafe fn modify"),
        "read-only CSR should NOT have modify()");
    assert!(!mcause_code.contains("fn _write"),
        "read-only CSR should NOT have _write asm");
    assert!(!mcause_code.contains("fn _set"),
        "read-only CSR should NOT have _set asm");
    assert!(!mcause_code.contains("fn _clear"),
        "read-only CSR should NOT have _clear asm");
}

#[test]
fn rvcsr_contains_fieldset_in_module() {
    let code = gen_to_string("csr_minimal.yaml", Mode::RvCsr, true);
    // Fieldset struct should be inside the CSR module (PascalCase after Sanitize transform)
    assert!(code.contains("pub struct Mstatus"), "should contain Mstatus fieldset struct");
    assert!(code.contains("pub struct Mie"), "should contain Mie fieldset struct");
    assert!(code.contains("pub struct Mcause"), "should contain Mcause fieldset struct");
}

#[test]
fn rvcsr_contains_per_field_set_clear() {
    let code = gen_to_string("csr_minimal.yaml", Mode::RvCsr, true);
    // mstatus has single-bit MIE field → should have set_mie() / clear_mie()
    assert!(code.contains("fn set_mie"), "should have set_mie() for single-bit field");
    assert!(code.contains("fn clear_mie"), "should have clear_mie() for single-bit field");
    assert!(code.contains("fn set_mpie"), "should have set_mpie() for single-bit field");
    assert!(code.contains("fn clear_mpie"), "should have clear_mpie() for single-bit field");
}

#[test]
fn rvcsr_multibit_field_set() {
    let code = gen_to_string("csr_minimal.yaml", Mode::RvCsr, true);
    // mstatus has multi-bit MPP field (2 bits, enum) → should have set_mpp(val)
    assert!(code.contains("fn set_mpp"), "should have set_mpp() for multi-bit enum field");
}

#[test]
fn rvcsr_no_common_module() {
    let code = gen_to_string("csr_minimal.yaml", Mode::RvCsr, true);
    // No common module needed in module-per-CSR style
    assert!(!code.contains("pub mod common"), "should NOT have common module");
    assert!(!code.contains("SealedCSR"), "should NOT have SealedCSR trait");
}

#[test]
fn rvcsr_contains_enum() {
    let code = gen_to_string("csr_minimal.yaml", Mode::RvCsr, true);
    assert!(code.contains("Machine") || code.contains("MACHINE"),
        "should contain PRIV_MODE enum variants");
}

// --- I2C device mode tests ---

#[test]
fn i2cdev_generates_output() {
    let code = gen_to_string("i2c_sensor.yaml", Mode::I2cDev, true);
    assert!(!code.is_empty());
}

#[test]
fn i2cdev_contains_reg_new() {
    let code = gen_to_string("i2c_sensor.yaml", Mode::I2cDev, true);
    assert!(code.contains("Reg :: new"), "I2C output should contain Reg::new() calls");
}

#[test]
fn i2cdev_contains_u8_addresses() {
    let code = gen_to_string("i2c_sensor.yaml", Mode::I2cDev, true);
    // CONFIG is at 0x01
    assert!(code.contains("1u8") || code.contains("0x01"),
        "I2C output should contain u8 register addresses");
}

#[test]
fn i2cdev_contains_register_array() {
    let code = gen_to_string("i2c_sensor.yaml", Mode::I2cDev, true);
    assert!(code.contains("ID") || code.contains("id"),
        "I2C output should contain ID register array");
    assert!(code.contains("n: usize") || code.contains("n : usize"),
        "register array should take index parameter");
}

#[test]
fn i2cdev_contains_access_types() {
    let code = gen_to_string("i2c_sensor.yaml", Mode::I2cDev, true);
    assert!(code.contains(":: R"), "should contain read-only access type");
    assert!(code.contains(":: RW"), "should contain read-write access type");
}

#[test]
fn i2cdev_contains_common_module() {
    let code = gen_to_string("i2c_sensor.yaml", Mode::I2cDev, true);
    assert!(code.contains("pub struct Reg"), "builtin common should contain Reg struct");
    assert!(code.contains("addr"), "I2C Reg should have addr field/method");
}

#[test]
fn i2cdev_contains_fieldset() {
    let code = gen_to_string("i2c_sensor.yaml", Mode::I2cDev, true);
    assert!(code.contains("sd") || code.contains("set_sd"),
        "should contain CONFIG fieldset accessors");
}

#[test]
fn i2cdev_contains_enum() {
    let code = gen_to_string("i2c_sensor.yaml", Mode::I2cDev, true);
    assert!(code.contains("Bits9") || code.contains("BITS_9"),
        "should contain RESOLUTION enum variants");
}

// --- Common path configuration tests ---

#[test]
fn custom_common_path_i2cdev() {
    let ir = yaml2pac::read_ir(fixture("i2c_sensor.yaml")).unwrap();
    let tmp = TempDir::new().unwrap();
    let out = tmp.path().join("out.rs");
    let opts = GenOptions {
        mode: Mode::I2cDev,
        builtin_common: true,
        common_module_path: Some("crate::regs::common".to_string()),
    };
    yaml2pac::gen_pac(ir, &out, &opts).unwrap();
    let code = std::fs::read_to_string(&out).unwrap();
    assert!(code.contains("crate :: regs :: common"),
        "should use custom common path");
}

// --- Multi-input merge test ---

#[test]
fn merge_multiple_yaml_inputs() {
    let ir1 = yaml2pac::read_ir(fixture("csr_minimal.yaml")).unwrap();
    let mut ir2 = yaml2pac::read_ir(fixture("csr_minimal.yaml")).unwrap();
    ir2.merge(ir1);
    // Should not panic — merged IR has same content (idempotent)
}
