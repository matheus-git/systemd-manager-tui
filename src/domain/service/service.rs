use zvariant::OwnedObjectPath;

pub struct Service {
    name: String,
    description: String,
    load_state: String,
    active_state: String,
    sub_state: String,
    followed: String,
    object_path: OwnedObjectPath,
    job_id: u32,
    job_type: String,
    job_object: OwnedObjectPath
}
