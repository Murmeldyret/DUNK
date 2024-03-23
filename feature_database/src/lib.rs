use diesel::pg::PgConnection;
use diesel::result::Error as DieselError;

pub mod models;
pub mod schema;
pub mod imagedb;

pub enum Keypoint<'a> {
    One(models::InsertKeypoint<'a>),
    Multiple(Vec<models::InsertKeypoint<'a>>),
}

pub trait KeypointDatabase {
    fn create_keypoint(conn: &mut PgConnection, keypoint: Keypoint) -> Result<(), DieselError>;
    fn read_keypoint_from_id(
        conn: &mut PgConnection,
        id: i32,
    ) -> Result<models::Keypoint, DieselError>;
    fn read_keypoints_from_image_id(
        conn: &mut PgConnection,
        image_id: i32,
    ) -> Result<Vec<models::Keypoint>, DieselError>;
    fn read_keypoints_from_lod(
        conn: &mut PgConnection,
        level: u32,
    ) -> Result<models::Keypoint, DieselError>;
    fn read_keypoints_from_coordinates(
        conn: &mut PgConnection,
        x_start: i32,
        y_start: i32,
        x_end: i32,
        y_end: i32,
    ) -> Result<models::Keypoint, DieselError>;
    fn delete_keypoint(conn: &mut PgConnection, id: i32) -> Result<(), DieselError>;
}

pub enum Descriptor<'a> {
    One(models::InsertDescriptor<'a>),
    Multiple(Vec<models::InsertDescriptor<'a>>),
}

pub trait DescriptorDatabase {
    fn create_descriptor(
        conn: &mut PgConnection,
        descriptor: Descriptor,
    ) -> Result<(), DieselError>;
    fn read_discriptor_from_id(
        conn: &mut PgConnection,
        id: i32,
    ) -> Result<models::Descriptor, DieselError>;
    fn read_discriptor_from_ids(
        conn: &mut PgConnection,
        ids: &[i32],
    ) -> Result<Vec<models::Descriptor>, DieselError>;
    fn delete_descriptor(conn: &mut PgConnection, id: i32) -> Result<(), DieselError>;
}