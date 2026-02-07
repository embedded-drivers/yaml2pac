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
fn rvcsr_contains_csr_marker_types() {
    let code = gen_to_string("csr_minimal.yaml", Mode::RvCsr, true);
    assert!(code.contains("CSR_MSTATUS"), "should generate CSR_MSTATUS marker type");
    assert!(code.contains("CSR_MIE"), "should generate CSR_MIE marker type");
    assert!(code.contains("CSR_MCAUSE"), "should generate CSR_MCAUSE marker type");
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
fn rvcsr_readonly_no_write_trait() {
    let code = gen_to_string("csr_minimal.yaml", Mode::RvCsr, true);
    // mcause is read-only: should impl SealedCSR + CSR but NOT SealedCSRWrite + CSRWrite
    assert!(code.contains("CSR_MCAUSE"), "should have CSR_MCAUSE marker");
    assert!(code.contains("SealedCSR for CSR_MCAUSE"), "read-only CSR should impl SealedCSR");
    assert!(!code.contains("SealedCSRWrite for CSR_MCAUSE"),
        "read-only CSR should NOT impl SealedCSRWrite");
    assert!(!code.contains("CSRWrite for CSR_MCAUSE"),
        "read-only CSR should NOT impl CSRWrite");
    // RW CSRs should still have write traits
    assert!(code.contains("SealedCSRWrite for CSR_MSTATUS"),
        "RW CSR should impl SealedCSRWrite");
    assert!(code.contains("CSRWrite for CSR_MSTATUS"),
        "RW CSR should impl CSRWrite");
    // No unimplemented!() anywhere
    assert!(!code.contains("unimplemented"),
        "should have zero unimplemented!() calls");
}

#[test]
fn rvcsr_contains_fieldset_accessors() {
    let code = gen_to_string("csr_minimal.yaml", Mode::RvCsr, true);
    assert!(code.contains("mie") || code.contains("set_mie"),
        "should contain MIE field accessor in MSTATUS fieldset");
}

#[test]
fn rvcsr_contains_common_module() {
    let code = gen_to_string("csr_minimal.yaml", Mode::RvCsr, true);
    assert!(code.contains("SealedCSR"), "builtin common should contain SealedCSR trait");
    assert!(code.contains("atomic_set") || code.contains("atomic_clear"),
        "builtin common should contain atomic operations");
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
fn custom_common_path_rvcsr() {
    let ir = yaml2pac::read_ir(fixture("csr_minimal.yaml")).unwrap();
    let tmp = TempDir::new().unwrap();
    let out = tmp.path().join("out.rs");
    let opts = GenOptions {
        mode: Mode::RvCsr,
        builtin_common: true,
        common_module_path: Some("crate::register::common".to_string()),
    };
    yaml2pac::gen_pac(ir, &out, &opts).unwrap();
    let code = std::fs::read_to_string(&out).unwrap();
    assert!(code.contains("crate :: register :: common"),
        "should use custom common path");
}

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
