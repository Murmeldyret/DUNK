use crate::schema::image::dsl::image as schema_image;
use diesel::pg::PgConnection;
use diesel::prelude::*;
use diesel::result::Error as DieselError;

use crate::models;

pub enum Image<'a> {
    One(models::InsertImage<'a>),
    Multiple(Vec<models::InsertImage<'a>>),
}

impl ImageDatabase for Image<'_> {
    fn create_image(conn: &mut PgConnection, input_image: Image) -> Result<(), DieselError> {
        match input_image {
            Image::One(single_image) => create_image_in_database(conn, &single_image)?,
            Image::Multiple(multiple_images) => {
                let result: Result<Vec<()>, DieselError> = multiple_images
                    .into_iter()
                    .map(|img| create_image_in_database(conn, &img))
                    .collect();

                    match result{
                    Ok(_) => return Ok(()),
                    Err(e) => return Err(e),
                    }
            }
        }
        Ok(())
    }

    fn read_image_from_id(conn: &mut PgConnection, id: i32) -> Result<models::Image, DieselError> {
        schema_image.find(id).select(models::Image::as_select()).first(conn)
    }

    fn find_images_from_dimensions(
        conn: &mut PgConnection,
        x_start: i32,
        y_start: i32,
        x_end: i32,
        y_end: i32,
        level_of_detail: i32,
    ) -> Result<Vec<i32>, DieselError> {
        schema_image.
        todo!()
    }

    fn find_images_from_lod(
        conn: &mut PgConnection,
        level_of_detail: i32,
    ) -> Result<Vec<i32>, DieselError> {
        todo!()
    }

    fn delete_image(conn: &mut PgConnection, id: i32) -> Result<(), DieselError> {
        todo!()
    }
}

fn create_image_in_database(
    connection: &mut PgConnection,
    insert_image: &models::InsertImage,
) -> Result<(), DieselError> {
    let result = diesel::insert_into(crate::schema::image::table)
        .values(insert_image)
        .returning(models::Image::as_returning())
        .get_result(connection);

    match result {
        Ok(_) => Ok(()),
        Err(e) => Err(e),
    }
}

pub trait ImageDatabase {
    fn create_image(conn: &mut PgConnection, image: Image) -> Result<(), DieselError>;
    fn read_image_from_id(conn: &mut PgConnection, id: i32) -> Result<models::Image, DieselError>;
    fn find_images_from_dimensions(
        conn: &mut PgConnection,
        x_start: i32,
        y_start: i32,
        x_end: i32,
        y_end: i32,
        level_of_detail: i32,
    ) -> Result<Vec<i32>, DieselError>;
    fn find_images_from_lod(
        conn: &mut PgConnection,
        level_of_detail: i32,
    ) -> Result<Vec<i32>, DieselError>;
    fn delete_image(conn: &mut PgConnection, id: i32) -> Result<(), DieselError>;
}

#[cfg(test)]
mod image_tests {
    use std::env;
    use std::sync::{Arc, Mutex};

    use super::*;
    use crate::schema::image::dsl::*;
    use dotenvy::dotenv;
    use once_cell::sync::Lazy;

    static DATABASE_LOCK: Lazy<Arc<Mutex<i32>>> = Lazy::new(|| Arc::new(Mutex::new(0)));
    static RESERVER_LOCK: Lazy<Arc<Mutex<i32>>> = Lazy::new(|| Arc::new(Mutex::new(0)));

    fn obtain_lock() -> std::sync::MutexGuard<'static, i32> {
        let _lock = RESERVER_LOCK.lock().unwrap();

        let lock = DATABASE_LOCK.lock();

        if lock.is_err() {
            return lock.unwrap_err().into_inner();
        }

        lock.unwrap()
    }

    fn setup_test_database() -> PgConnection {
        dotenv().ok();

        let database_url =
            env::var("DATABASE_URL").expect("DATABASE_URL must be set for tests to work");

        let mut connection = Connection::establish(&database_url)
            .unwrap_or_else(|_| panic!("Error connecting to {}", database_url));

        // TODO: This can be done smarter :)
        diesel::sql_query("DELETE FROM descriptor")
            .execute(&mut connection)
            .unwrap();
        diesel::sql_query("ALTER SEQUENCE descriptor_id_seq RESTART WITH 1")
            .execute(&mut connection)
            .unwrap();
        diesel::sql_query("DELETE FROM keypoint")
            .execute(&mut connection)
            .unwrap();
        diesel::sql_query("ALTER SEQUENCE keypoint_id_seq RESTART WITH 1")
            .execute(&mut connection)
            .unwrap();
        diesel::sql_query("DELETE FROM image")
            .execute(&mut connection)
            .unwrap();
        diesel::sql_query("ALTER SEQUENCE image_id_seq RESTART WITH 1")
            .execute(&mut connection)
            .unwrap();

        connection
    }

    #[test]
    fn image_creation() {
        let _lock = obtain_lock();
        let connection = &mut setup_test_database();

        let insert_image = models::InsertImage {
            x_start: &0,
            y_start: &0,
            x_end: &10,
            y_end: &10,
            level_of_detail: &1,
        };

        let inserted_image = Image::One(insert_image);

        Image::create_image(connection, inserted_image).expect("Could not add image to database");

        let fetched_image: models::Image = image
            .find(1)
            .select(models::Image::as_select())
            .first(connection)
            .expect("Could not find created image");

        assert_eq!(fetched_image.x_start, *insert_image.x_start);
        assert_eq!(fetched_image.y_start, *insert_image.y_start);
        assert_eq!(fetched_image.x_end, *insert_image.x_end);
        assert_eq!(fetched_image.y_end, *insert_image.y_end);
        assert_eq!(fetched_image.level_of_detail, *insert_image.level_of_detail);
    }

    #[test]
    fn image_fetching_id() {
        let _lock = obtain_lock();
        let connection = &mut setup_test_database();

        let insert_image = models::InsertImage {
            x_start: &0,
            y_start: &0,
            x_end: &10,
            y_end: &10,
            level_of_detail: &1,
        };

        diesel::insert_into(crate::schema::image::table)
            .values(&insert_image)
            .returning(models::Image::as_returning())
            .get_result(connection)
            .expect("Error saving new image");

        let fetched_image =
            Image::read_image_from_id(connection, 1).expect("Could not read image from database");

        assert_eq!(fetched_image.x_start, *insert_image.x_start);
        assert_eq!(fetched_image.y_start, *insert_image.y_start);
        assert_eq!(fetched_image.x_end, *insert_image.x_end);
        assert_eq!(fetched_image.y_end, *insert_image.y_end);
        assert_eq!(fetched_image.level_of_detail, *insert_image.level_of_detail);
    }

    #[test]
    fn image_fetching_id_not_available() {
        let _lock = obtain_lock();
        let connection = &mut setup_test_database();

        let fetched_image = Image::read_image_from_id(connection, 1);

        assert!(fetched_image.is_err_and(|e| e.eq(&DieselError::NotFound)));
    }

    #[test]
    fn image_fetching_dimensions() {
        let _lock = obtain_lock();
        let connection = &mut setup_test_database();

        // TODO: Make a generator of images
        let insert_images = vec![
            models::InsertImage {
                x_start: &0,
                y_start: &0,
                x_end: &9,
                y_end: &9,
                level_of_detail: &1,
            },
            models::InsertImage {
                x_start: &10,
                y_start: &0,
                x_end: &19,
                y_end: &9,
                level_of_detail: &1,
            },
            models::InsertImage {
                x_start: &0,
                y_start: &10,
                x_end: &9,
                y_end: &19,
                level_of_detail: &1,
            },
            models::InsertImage {
                x_start: &10,
                y_start: &10,
                x_end: &19,
                y_end: &19,
                level_of_detail: &1,
            },
        ];

        insert_images.into_iter().for_each(|single_image| {
            diesel::insert_into(crate::schema::image::table)
                .values(&single_image)
                .returning(models::Image::as_returning())
                .get_result(connection)
                .expect("Error saving new image");
        });

        let fetched_image_ids = Image::find_images_from_dimensions(connection, 3, 0, 15, 7, 1);

        assert!(fetched_image_ids.is_ok_and(|ids| ids.contains(&1) && ids.contains(&2)));
    }

    #[test]
    fn images_fetched_from_lod() {
        let _lock = obtain_lock();
        let connection = &mut setup_test_database();

        // TODO: Make a generator of images
        let insert_images = vec![
            models::InsertImage {
                x_start: &0,
                y_start: &0,
                x_end: &9,
                y_end: &9,
                level_of_detail: &1,
            },
            models::InsertImage {
                x_start: &10,
                y_start: &0,
                x_end: &19,
                y_end: &9,
                level_of_detail: &1,
            },
            models::InsertImage {
                x_start: &0,
                y_start: &10,
                x_end: &9,
                y_end: &19,
                level_of_detail: &1,
            },
            models::InsertImage {
                x_start: &10,
                y_start: &10,
                x_end: &19,
                y_end: &19,
                level_of_detail: &1,
            },
        ];

        insert_images.into_iter().for_each(|single_image| {
            diesel::insert_into(crate::schema::image::table)
                .values(&single_image)
                .returning(models::Image::as_returning())
                .get_result(connection)
                .expect("Error saving new image");
        });

        let image_ids = Image::find_images_from_lod(connection, 1);

        assert!(image_ids.is_ok_and(|ids| (1..=4)
            .into_iter()
            .fold(false, |acc, i| acc || ids.contains(&i))));
    }

    #[test]
    fn image_deletion() {
        let _lock = obtain_lock();
        let connection = &mut setup_test_database();

        let insert_images = vec![
            models::InsertImage {
                x_start: &0,
                y_start: &0,
                x_end: &9,
                y_end: &9,
                level_of_detail: &1,
            },
            models::InsertImage {
                x_start: &10,
                y_start: &0,
                x_end: &19,
                y_end: &9,
                level_of_detail: &1,
            },
            models::InsertImage {
                x_start: &0,
                y_start: &10,
                x_end: &9,
                y_end: &19,
                level_of_detail: &1,
            },
            models::InsertImage {
                x_start: &10,
                y_start: &10,
                x_end: &19,
                y_end: &19,
                level_of_detail: &1,
            },
        ];

        insert_images.into_iter().for_each(|single_image| {
            diesel::insert_into(crate::schema::image::table)
                .values(&single_image)
                .returning(models::Image::as_returning())
                .get_result(connection)
                .expect("Error saving new image");
        });

        let result = Image::delete_image(connection, 1);

        let db_result = image
            .select(models::Image::as_select())
            .load(connection)
            .expect("Error loading images");

        assert!(result.is_ok());
        assert_eq!(db_result.len(), 3);
    }
}
