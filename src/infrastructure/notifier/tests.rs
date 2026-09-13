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

#[test]
fn decodes_escaped_at_sign_and_underscore() {
    assert_eq!(
        decode_unit_path("/org/freedesktop/systemd1/unit/worker_40blue_5fteam_2eservice"),
        "worker@blue_team.service"
    );
}

#[test]
fn returns_only_the_final_object_path_segment() {
    assert_eq!(decode_unit_path("/a/b/c/demo_2eservice"), "demo.service");
}

#[test]
fn handles_an_empty_path_without_panicking() {
    assert_eq!(decode_unit_path(""), "");
}
