use zvariant::OwnedObjectPath;

pub struct Service {
   pub name: String,
   pub description: String,
   pub load_state: String,
   pub active_state: String,
   pub sub_state: String,
   pub followed: String,
   pub file_state: String,
   pub object_path: OwnedObjectPath,
   pub job_id: u32,
   pub job_type: String,
   pub job_object: OwnedObjectPath
} 
