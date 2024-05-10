use crate::models;
use diesel::pg::PgConnection;
use diesel::prelude::*;
use diesel::result::Error as DieselError;

#[derive(Debug)]
pub enum Errors {
    Gdal(gdal::errors::GdalError),
    Diesel(DieselError),
}

pub mod geotransform {
    use super::*;
    use crate::schema::geotransform::dsl;
    use gdal::GeoTransform;
    use gdal::GeoTransformEx;

    /// Stores a geotransform in the dataset. The name is not choosable by the user.
    /// The name of the transform should be either "dataset" or "elevation".
    pub fn create_geotransform(
        conn: &mut PgConnection,
        name: &str,
        transform: GeoTransform,
    ) -> Result<(), DieselError> {
        let insert_transform = models::InsertGeoTransform {
            dataset_name: name,
            transform: &transform,
        };

        diesel::insert_into(crate::schema::geotransform::table)
            .values(insert_transform)
            .execute(conn)?;

        Ok(())
    }

    fn read_geotransform(conn: &mut PgConnection, name: &str) -> Result<GeoTransform, DieselError> {
        let transform: models::GeoTransform = dsl::geotransform
            .filter(dsl::dataset_name.eq(name))
            .select(models::GeoTransform::as_select())
            .first(conn)?;

        // If the transform is in the database then everything works and unwrap is alright.
        let transform: Vec<f64> = transform
            .transform
            .iter()
            .map(|element| element.expect("Failed unwrap geotransform"))
            .collect();
        let transform: GeoTransform = transform.try_into().unwrap();

        Ok(transform)
    }

    pub fn get_world_coordinates(
        conn: &mut PgConnection,
        x: f64,
        y: f64,
    ) -> Result<(f64, f64, f64), Errors> {
        let transform = read_geotransform(conn, "dataset").map_err(|e| Errors::Diesel(e))?;

        let coordinates = transform.apply(x, y);

        let elevation_transform = match read_geotransform(conn, "elevation") {
            Ok(transform) => transform,
            Err(e) => return Ok((coordinates.0, coordinates.1, 0.0)),
        };

        let inv_ele = elevation_transform.invert().unwrap();

        let elevation_pixels = inv_ele.apply(coordinates.0, coordinates.1);

        let height = super::elevation::get_elevation(conn, elevation_pixels.0, elevation_pixels.1)
            .map_err(|e| Errors::Diesel(e))?;

        Ok((coordinates.0, coordinates.1, height))
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use crate::db_helpers::{obtain_lock, setup_database};
        use crate::schema::geotransform::dsl;

        #[test]
        fn add_geotransform_to_database() {
            let _lock = obtain_lock();
            let connection = &mut setup_database();

            let transform: [f64; 6] = [1.0, 2.0, 3.0, 4.0, 5.0, 6.0];

            create_geotransform(connection, "dataset", transform).unwrap();

            let fetched_transform: models::GeoTransform = dsl::geotransform
                .find(1)
                .select(models::GeoTransform::as_select())
                .first(connection)
                .unwrap();

            let fetched_transform: Vec<f64> = fetched_transform
                .transform
                .iter()
                .map(|e| e.unwrap())
                .collect();

            assert_eq!(fetched_transform, transform);
        }

        #[test]
        fn read_geotransform_from_database() {
            let _lock = obtain_lock();
            let connection = &mut setup_database();

            let transform: [f64; 6] = [1.0, 2.0, 3.0, 4.0, 5.0, 6.0];

            let insert_tranform = models::InsertGeoTransform {
                dataset_name: "dataset",
                transform: &transform,
            };

            diesel::insert_into(crate::schema::geotransform::table)
                .values(insert_tranform)
                .execute(connection)
                .unwrap();

            let fetched_transform = read_geotransform(connection, "dataset").unwrap();

            assert_eq!(fetched_transform, transform);
        }

        #[test]
        fn read_geotransform_from_empty() {
            let _lock = obtain_lock();
            let connection = &mut setup_database();

            let fetched_transform = read_geotransform(connection, "dataset");

            assert!(fetched_transform.is_err());
        }
    }
}

pub mod elevation {
    use super::*;
    use crate::schema::{elevation, elevation_properties};
    use gdal::Dataset;
    pub fn add_elevation_data(conn: &mut PgConnection, dataset: &Dataset) -> Result<(), Errors> {
        let rasterband = dataset.rasterband(1).map_err(|e| Errors::Gdal(e))?;
        let dimensions = rasterband.size();

        let insert_properties = models::InsertElevationProperties {
            x_size: &(dimensions.0 as i32),
            y_size: &(dimensions.1 as i32),
        };

        let image: Vec<f64> = rasterband
            .read_as((0, 0), dimensions, dimensions, None)
            .map_err(|e| Errors::Gdal(e))?
            .data;

        let insert_image: Vec<models::InsertElevation> = image
            .iter()
            .map(|pixel| models::InsertElevation { height: pixel })
            .collect();

        let upload_limit = insert_image.len() / 65535;

        // Diesel has a limit of max 65535 parameters :)
        for i in 0..upload_limit {
            let insert_vec = insert_image[65535 * i..65535 * (i + 1)].to_vec();
            diesel::insert_into(crate::schema::elevation::table)
                .values(insert_vec)
                .execute(conn)
                .map_err(|e| Errors::Diesel(e))?;
        }
        let insert_vec = insert_image[65535 * upload_limit..insert_image.len()].to_vec();
        diesel::insert_into(crate::schema::elevation::table)
            .values(insert_vec)
            .execute(conn)
            .map_err(|e| Errors::Diesel(e))?;

        diesel::insert_into(crate::schema::elevation_properties::table)
            .values(insert_properties)
            .execute(conn)
            .map_err(|e| Errors::Diesel(e))?;

        Ok(())
    }

    pub fn get_elevation(conn: &mut PgConnection, x: f64, y: f64) -> Result<f64, DieselError> {
        let properties = elevation_properties::dsl::elevation_properties
            .select(models::ElevationProperties::as_select())
            .first(conn)?;

        let height: models::Elevation = elevation::dsl::elevation
            .find(y.round() as i32 * properties.x_size + x.round() as i32 + 1)
            .select(models::Elevation::as_select())
            .first(conn)?;

        Ok(height.height)
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use crate::db_helpers::{obtain_lock, setup_database};
        use gdal::Dataset;
        use std::env;
        #[test]
        fn add_elevation_data_to_db() {
            let _lock = obtain_lock();
            let connection = &mut setup_database();
            let himmel_x = 549;
            let himmel_y = 1074;
            let mut current_dir = env::current_dir().expect("Current directory not set.");

            current_dir.pop();
            let path = "resources/test/Geotiff/Elevation_test/elevation/Copernicus_DSM_COG_30_N56_00_E009_00_DEM.tif";
            current_dir.push(path);
            let ds = Dataset::open(current_dir).unwrap();

            add_elevation_data(connection, &ds).unwrap();

            let elevation_db: models::Elevation = crate::schema::elevation::dsl::elevation
                .find(himmel_y * 800 + himmel_x + 1)
                .select(models::Elevation::as_select())
                .first(connection)
                .unwrap();

            dbg!(&elevation_db.height);

            assert!((elevation_db.height - 147.0).abs() < 2.0)
        }

        #[test]
        fn get_elevation_data_from_db() {
            let _lock = obtain_lock();
            let connection = &mut setup_database();
            let himmel_x = 549.04;
            let himmel_y = 1073.7972;
            let mut current_dir = env::current_dir().expect("Current directory not set.");

            current_dir.pop();
            let path = "resources/test/Geotiff/Elevation_test/elevation/Copernicus_DSM_COG_30_N56_00_E009_00_DEM.tif";
            current_dir.push(path);
            let ds = Dataset::open(current_dir).unwrap();

            add_elevation_data(connection, &ds).unwrap(); // I know it makes it dependent on another function but it's a nightmare to do it directly in diesel, soo......

            let elevation_db = get_elevation(connection, himmel_x, himmel_y).unwrap();

            dbg!(&elevation_db);

            assert!((elevation_db - 147.0).abs() < 2.0)
        }
    }
}
