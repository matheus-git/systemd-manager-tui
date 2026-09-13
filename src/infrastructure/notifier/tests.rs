use super::decode_unit_path;

#[test]
fn decodes_systemd_object_path_escapes() {
    assert_eq!(
        decode_unit_path("/org/freedesktop/systemd1/unit/foo_2dbar_2eservice"),
        "foo-bar.service"
    );
}

#[test]
fn preserves_invalid_escape_sequences() {
    assert_eq!(decode_unit_path("/unit/foo_zzbar"), "foo_zzbar");
}

