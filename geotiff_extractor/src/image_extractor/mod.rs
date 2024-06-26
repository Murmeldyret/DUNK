use gdal::errors;
use gdal::raster::{ColorInterpretation, RasterCreationOption, StatisticsMinMax};
use gdal::{Dataset, GeoTransformEx};

use std::path::PathBuf;

use gdal::raster::ResampleAlg;

use gdal::programs::raster::build_vrt;

#[cfg(test)]
use mockall::{automock, predicate::*};

const GAMMA_VALUE: f32 = 1.0 / 2.2;
const U8_MAX: f32 = u8::MAX as f32;

// A struct for handling raw datasets from disk in Geotiff format
pub struct RawDataset {
    pub datasets: Vec<Dataset>,
}

#[derive(Debug, PartialEq)]
pub struct DatasetOptions {
    pub scaling: (usize, usize),
    pub red_band_index: isize,
    pub green_band_index: isize,
    pub blue_band_index: isize,
}

impl DatasetOptions {
    pub fn builder() -> DatasetOptionsBuilder {
        DatasetOptionsBuilder::default()
    }
}

#[derive(Default)]
pub struct DatasetOptionsBuilder {
    pub scaling: Option<(usize, usize)>,
    pub red_band_index: Option<isize>,
    pub green_band_index: Option<isize>,
    pub blue_band_index: Option<isize>,
}

impl DatasetOptionsBuilder {
    pub fn new() -> DatasetOptionsBuilder {
        DatasetOptionsBuilder::default()
    }

    pub fn set_scaling(mut self, x: usize, y: usize) -> DatasetOptionsBuilder {
        self.scaling = Some((x, y));
        self
    }

    pub fn set_band_indexes(
        mut self,
        red: isize,
        green: isize,
        blue: isize,
    ) -> DatasetOptionsBuilder {
        self.red_band_index = Some(red);
        self.green_band_index = Some(green);
        self.blue_band_index = Some(blue);
        self
    }

    pub fn build(self) -> DatasetOptions {
        DatasetOptions {
            scaling: self.scaling.unwrap_or((1024, 1024)),
            red_band_index: self.red_band_index.unwrap_or(1),
            green_band_index: self.green_band_index.unwrap_or(2),
            blue_band_index: self.blue_band_index.unwrap_or(3),
        }
    }
}

// The converted mosaic dataset in COG format
pub struct MosaicedDataset {
    pub dataset: Dataset,
    pub options: DatasetOptions,
    pub min_max: Option<BandsMinMax>,
    pub elevation: Option<Dataset>,
}

#[cfg_attr(test, automock)]
pub trait Datasets {
    fn import_datasets(paths: &str) -> Result<RawDataset, errors::GdalError>;
    fn to_mosaic_dataset(&self, output_path: &str) -> Result<MosaicedDataset, errors::GdalError>;
    fn to_vrt_dataset(&self) -> Result<MosaicedDataset, errors::GdalError>;
}

#[cfg_attr(test, automock)]
pub trait MosaicDataset {
    fn import_mosaic_dataset(path: &str) -> Result<MosaicedDataset, errors::GdalError>;
    fn datasets_min_max(&mut self) -> Result<BandsMinMax, errors::GdalError>;
    fn get_dimensions(&self) -> Result<(i64, i64), errors::GdalError>;
    fn set_scaling(&self, dimensions: (usize, usize));
    fn to_rgb(
        &mut self,
        window: (isize, isize),
        window_size: (usize, usize),
        size: (usize, usize),
    ) -> Result<Vec<rgb::RGBA8>, errors::GdalError>;
    fn detect_nodata(&self) -> bool;
    fn fill_nodata(&mut self);
    fn set_bands(&self, red_band: isize, green_band: isize, blue_band: isize);
    fn set_elevation_dataset(&mut self, path: &str, output_path: &str) -> Result<(), errors::GdalError>;
    fn get_world_coordinates(&self, x: f64, y: f64) -> Result<(f64,f64,f64), errors::GdalError>;
}

#[derive(Debug, Clone, Copy)]
pub struct BandsMinMax {
    pub red_min: f64,
    pub red_max: f64,
    pub green_min: f64,
    pub green_max: f64,
    pub blue_min: f64,
    pub blue_max: f64,
}

#[derive(Debug, Clone)]
pub enum PixelConversion {
    GammaOutOfRange,
    FloatToIntegerError,
    NotANumber,
}

impl Datasets for RawDataset {
    /// The function will import multiple datasets from a vector of paths.
    /// Providing the function of a slice of [Path]s then it will return a [Result<RawDataset>]
    fn import_datasets(path: &str) -> Result<RawDataset, errors::GdalError> {
        let unwrapped_data = match dataset_from_folder(path) {
            Ok(value) => value,
            Err(value) => return Err(value),
        };
        Ok(RawDataset {
            datasets: unwrapped_data,
        })
    }

    /// Returns a mosaic dataset that is a combined version of the [RawDataset] dataset provided.
    fn to_mosaic_dataset(&self, output_path: &str) -> Result<MosaicedDataset, errors::GdalError> {
        let mut vrt_path = PathBuf::from(&output_path);
        let mut cog_path = PathBuf::from(output_path);

        vrt_path.push("dataset.vrt");
        cog_path.push("dataset.tif");

        let result_vrt = build_vrt(Some(vrt_path.as_path()), &self.datasets, None)?;

        let create_options = creation_options();

        let mosaic = result_vrt.create_copy(
            &gdal::DriverManager::get_driver_by_name("COG")?,
            cog_path,
            &create_options,
        )?;

        Ok(MosaicedDataset {
            dataset: mosaic,
            options: DatasetOptionsBuilder::new().build(),
            min_max: None,
            elevation: None,
        })
    }

    fn to_vrt_dataset(&self) -> Result<MosaicedDataset, errors::GdalError> {
        let vrt = build_vrt(None, &self.datasets, None)?;

        Ok(MosaicedDataset {
            dataset: vrt,
            options: DatasetOptionsBuilder::new().build(),
            min_max: None,
            elevation: None,
        })
    }
}

fn dataset_from_folder(path: &str) -> Result<Vec<Dataset>, errors::GdalError> {
    let directory = match std::fs::read_dir(path) {
        Ok(dir) => dir,
        Err(_) => {
            return Err(errors::GdalError::NullPointer {
                method_name: "read_dir",
                msg: String::from("No directory"),
            })
        }
    };
    let ds = directory
        .into_iter()
        .map(|p| Dataset::open(p.unwrap().path()))
        .collect();
    let unwrapped_data = match ds {
        Ok(data) => data,
        Err(e) => return Err(e),
    };
    Ok(unwrapped_data)
}

impl MosaicDataset for MosaicedDataset {
    fn datasets_min_max(&mut self) -> Result<BandsMinMax, errors::GdalError> {
        if self.min_max.is_some() {
            return Ok(self.min_max.expect("Could not return min_max"));
        }

        let dataset = &self.dataset;

        let min_max: Vec<StatisticsMinMax> = (1..4)
            .map(|i| {
                let ds_min_max = dataset.rasterband(i)?.compute_raster_min_max(true)?;
                Ok::<StatisticsMinMax, errors::GdalError>(StatisticsMinMax {
                    min: ds_min_max.min,
                    max: ds_min_max.max,
                })
            })
            .collect::<Result<Vec<StatisticsMinMax>, errors::GdalError>>()?;

        let min_max = BandsMinMax {
            red_min: min_max[0].min,
            red_max: min_max[0].max,
            green_min: min_max[1].min,
            green_max: min_max[1].max,
            blue_min: min_max[2].min,
            blue_max: min_max[2].max,
        };

        self.min_max = Some(min_max);

        Ok(min_max)
    }

    fn get_dimensions(&self) -> Result<(i64, i64), errors::GdalError> {
        let dimensions = self.dataset.raster_size();

        Ok((dimensions.0 as i64, dimensions.1 as i64))
    }

    fn set_scaling(&self, _dimensions: (usize, usize)) {
        todo!()
    }

    fn to_rgb(
        &mut self,
        window: (isize, isize),
        window_size: (usize, usize),
        size: (usize, usize),
    ) -> Result<Vec<rgb::RGBA8>, errors::GdalError> {
        let mut red_band = self.dataset.rasterband(1)?;
        red_band.set_color_interpretation(ColorInterpretation::RedBand)?;
        let red_converted = extract_band(&red_band, window, window_size, size)?;

        let mut green_band = self.dataset.rasterband(2)?;
        green_band.set_color_interpretation(ColorInterpretation::GreenBand)?;
        let green_converted = extract_band(&green_band, window, window_size, size)?;

        let mut blue_band = self.dataset.rasterband(3)?;
        blue_band.set_color_interpretation(ColorInterpretation::BlueBand)?;
        let blue_converted = extract_band(&blue_band, window, window_size, size)?;

        let bands = vec![red_converted, green_converted, blue_converted];

        let min_max = self.datasets_min_max()?;

        let combined_bands = match band_merger(&bands, &min_max) {
            Ok(combined_bands) => combined_bands,
            Err(_) => return Err(errors::GdalError::CastToF64Error),
        };

        Ok(combined_bands)
    }

    fn detect_nodata(&self) -> bool {
        todo!()
    }

    fn fill_nodata(&mut self) {
        todo!()
    }

    fn import_mosaic_dataset(path: &str) -> Result<MosaicedDataset, errors::GdalError> {
        let ds = Dataset::open(path)?;

        Ok(MosaicedDataset {
            dataset: ds,
            options: DatasetOptionsBuilder::new().build(),
            min_max: None,
            elevation: None,
        })
    }

    fn set_bands(&self, _red_band: isize, _green_band: isize, _blue_band: isize) {
        todo!()
    }

    fn set_elevation_dataset(&mut self, path: &str, output_path: &str) -> Result<(), errors::GdalError> {
        let ds = match dataset_from_folder(path) {
            Ok(dataset) => dataset,
            Err(e) => return Err(e),
        };

        let mut vrt_path = PathBuf::from(&output_path);

        vrt_path.push("elevation.vrt");

        let result_vrt = build_vrt(Some(vrt_path.as_path()), &ds, None)?;

        self.elevation = Some(result_vrt);

        Ok(())
    }

    fn get_world_coordinates(&self, x: f64, y: f64) -> Result<(f64,f64,f64), errors::GdalError> {
        let geotransform = self.dataset.geo_transform()?;

        let coordinates = geotransform.apply(x, y);

        let elevation_transform = match &self.elevation {
            Some(dataset) => dataset.geo_transform()?,
            None => return Ok((coordinates.0, coordinates.1, 0.0))
        };

        let invers_elev = elevation_transform.invert()?;

        let elev_pixels = invers_elev.apply(coordinates.0, coordinates.1);

        let elevation = self.elevation.as_ref().unwrap().rasterband(1)?.read_as::<f64>((elev_pixels.0.round() as isize, elev_pixels.1.round() as isize), (1,1), (1,1), None)?.data;

        Ok((coordinates.0, coordinates.1, elevation[0]))

    }
}

fn extract_band(
    band: &gdal::raster::RasterBand,
    window: (isize, isize),
    window_size: (usize, usize),
    size: (usize, usize),
) -> Result<Vec<f32>, errors::GdalError> {
    let band_vec = band
        .read_as::<f32>(window, window_size, size, Some(ResampleAlg::Lanczos))?
        .data;

    Ok(band_vec)
}

/// Assumes a [Vec<Vec<u8>>] in the order R,G,B.
fn band_merger(
    bands: &[Vec<f32>],
    min_max: &BandsMinMax,
) -> Result<Vec<rgb::RGBA8>, PixelConversion> {
    let mut combined_bands: Vec<rgb::RGBA8> = Vec::with_capacity(bands[0].len() * 4);

    for i in 0..bands[0].len() {
        let mut alpha = 255;

        if bands.iter().fold(true, |acc, band| band[i].is_nan() && acc) {
            alpha = 0;
        }

        let red =
            f32_to_u8(bands[0][i], min_max.red_min as f32, min_max.red_max as f32).unwrap_or(0);
        let green = f32_to_u8(
            bands[1][i],
            min_max.green_min as f32,
            min_max.green_max as f32,
        )
        .unwrap_or(0);
        let blue = f32_to_u8(
            bands[2][i],
            min_max.blue_min as f32,
            min_max.blue_max as f32,
        )
        .unwrap_or(0);

        combined_bands.push(rgb::RGBA8::new(red, green, blue, alpha));
    }

    Ok(combined_bands)
}

fn creation_options() -> Vec<RasterCreationOption<'static>> {
    let create_options = vec![
        RasterCreationOption {
            key: "COMPRESS",
            value: "ZSTD",
        },
        RasterCreationOption {
            key: "PREDICTOR",
            value: "YES",
        },
        RasterCreationOption {
            key: "BIGTIFF",
            value: "YES",
        },
        RasterCreationOption {
            key: "NUM_THREADS",
            value: "ALL_CPUS",
        },
    ];
    create_options
}

fn gamma_correction(input_value: f32) -> Result<f32, PixelConversion> {
    if !(0.0..=1.0).contains(&input_value) {
        return Err(PixelConversion::GammaOutOfRange);
    }

    Ok(input_value.powf(GAMMA_VALUE))
}

fn f32_to_u8(input_value: f32, min: f32, max: f32) -> Result<u8, PixelConversion> {
    if input_value.is_nan() {
        return Err(PixelConversion::NotANumber);
    }

    let float = (input_value - min) / (max - min);

    let normal_float = gamma_correction(float)?;

    let converted_integer = (normal_float * U8_MAX).round() as u8;

    Ok(converted_integer)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    use tempfile::tempdir;

    #[test]
    fn import_dataset_missing() {
        let wrong_paths = String::from("/Nowhere");

        let result = RawDataset::import_datasets(&wrong_paths);

        assert!(result.is_err());
    }

    #[test]
    fn import_dataset_exists() {
        let mut current_dir = env::current_dir().expect("Current directory not set.");

        current_dir.pop();

        dbg!(&current_dir);

        let mut path = current_dir.clone();
        path.push("resources/test/Geotiff/gdal_tests");

        dbg!(&path.to_string_lossy());

        let result = RawDataset::import_datasets(&path.to_string_lossy());

        assert!(result.unwrap().datasets[0].raster_size() == (7309, 4322));
    }

    #[test]
    fn combining_dataset() {
        //TODO: The test should cleanup after itself. Maybe use tmp files.
        let mut current_dir = env::current_dir().expect("Current directory not set.");

        current_dir.pop();

        let mut path1 = current_dir.clone();
        path1.push("resources/test/Geotiff/gdal_tests/MOSAIC-0000018944-0000037888.tif");

        let mut path2 = current_dir.clone();
        path2.push("resources/test/Geotiff/gdal_tests/MOSAIC-0000018944-0000018944.tif");

        let mut output_path = current_dir.clone();

        output_path.push("resources/dataset");

        let paths = vec![path1, path2];

        let datasets: Vec<Dataset> = paths
            .into_iter()
            .map(|p| Dataset::open(p.as_path()))
            .collect::<Result<Vec<Dataset>, errors::GdalError>>()
            .expect("Could not open test files.");

        let datasets = RawDataset { datasets };

        let result = datasets.to_mosaic_dataset(output_path.to_str().unwrap());

        assert!(result.is_ok());
    }

    #[test]
    fn find_min_max_dataset() {
        let mut current_dir = env::current_dir().expect("Current directory not set.");

        current_dir.pop();

        current_dir.push("resources/test/Geotiff/gdal_tests/MOSAIC-0000018944-0000037888.tif");

        dbg!(&current_dir);

        let ds = Dataset::open(current_dir.as_path()).expect("Could not open dataset");

        let mut dataset = MosaicedDataset {
            dataset: ds,
            options: DatasetOptionsBuilder::new().build(),
            min_max: None,
            elevation: None
        };

        let result = MosaicDataset::datasets_min_max(&mut dataset);

        assert_eq!(
            0.0017,
            (result.unwrap().red_min * 10000.0).round() / 10000.0
        );
    }

    #[test]
    fn gamma_correct_input() {
        let input_value = 0.5;

        let output_value = gamma_correction(input_value);

        dbg!(&output_value.clone().unwrap());

        assert!(output_value.is_ok_and(|result| result == 0.7297401));
    }

    #[test]
    fn gamma_value_above_1() {
        let input_value = 1.5;

        let output_value = gamma_correction(input_value);

        assert!(output_value.is_err());
    }

    #[test]
    fn gamma_value_below_0() {
        let input_value = -0.5;

        let output_value = gamma_correction(input_value);

        assert!(output_value.is_err());
    }

    //TODO: Gamma skal mockes, men det gidder jeg ikke lige nu.
    #[test]
    fn convert_f32_to_u8_success() {
        let input_value = 0.2;
        let min = 0.1;
        let max = 0.3;

        let output_value = f32_to_u8(input_value, min, max);

        assert!(output_value.is_ok_and(|result| result == 186));
    }

    #[test]
    fn convert_f32_to_u8_nan() {
        let input_value = f32::NAN;
        let min = 0.1;
        let max = 0.3;

        let output_value = f32_to_u8(input_value, min, max);

        assert!(output_value.is_err_and(|error| match error {
            PixelConversion::NotANumber => true,
            _ => false,
        }));
    }

    #[test]
    fn dataset_to_rgb() {
        let mut current_dir = env::current_dir().expect("Current directory not set.");

        current_dir.pop();

        current_dir
            .push("resources/test/Geotiff/ESA/ESA_WorldCover_10m_2021_v200_N54E009_S2RGBNIR.tif");

        let ds = Dataset::open(current_dir.as_path()).expect("Could not open dataset");

        let mut dataset = MosaicedDataset {
            dataset: ds,
            options: DatasetOptionsBuilder::new().build(),
            min_max: None,
            elevation: None,
        };

        let window_size = dataset.dataset.raster_size();

        let image_rgba: Result<Vec<rgb::RGBA8>, _> = dataset.to_rgb((0, 0), window_size, (20, 20));

        assert!(image_rgba.is_ok_and(|image_vec| image_vec.len() == 20 * 20));
    }

    #[test]
    fn band_extraction() {
        let mut current_dir = env::current_dir().expect("Current directory not set.");

        current_dir.pop();

        current_dir.push("resources/test/Geotiff/gdal_tests/MOSAIC-0000018944-0000037888.tif");

        dbg!(&current_dir);

        let ds = Dataset::open(current_dir.as_path()).expect("Could not open dataset");

        let dataset = MosaicedDataset {
            dataset: ds,
            options: DatasetOptionsBuilder::new().build(),
            min_max: None,
            elevation: None,
        };

        let red_band = dataset.dataset.rasterband(1).expect("Could not open band");

        let band_vec =
            extract_band(&red_band, (0, 0), dataset.dataset.raster_size(), (20, 20)).unwrap();

        // dbg!(&band_vec);

        assert_eq!(band_vec.len(), 400);
    }

    #[test]
    fn merging_bands() {
        let red = vec![0.0, 0.5, 1.0];
        let green = vec![0.0, 0.5, 1.0];
        let blue = vec![0.0, 0.5, 1.0];

        let combined = vec![red, green, blue];

        let min_max = BandsMinMax {
            red_min: -1.0,
            red_max: 2.0,
            green_min: -1.0,
            green_max: 2.0,
            blue_min: -1.0,
            blue_max: 2.0,
        };

        let merged_bands = band_merger(&combined, &min_max).expect("Could not merge bands");

        assert_eq!(merged_bands.len(), 3);
        assert_eq!(merged_bands[0].r, 155);
    }

    #[test]
    fn option_builder_test() {
        let dataset_options = DatasetOptions {
            scaling: (2048, 1024),
            red_band_index: 4,
            green_band_index: 3,
            blue_band_index: 2,
        };

        let dataset_options_from_builder: DatasetOptions = DatasetOptionsBuilder::new()
            .set_scaling(2048, 1024)
            .set_band_indexes(4, 3, 2)
            .build();

        assert_eq!(dataset_options, dataset_options_from_builder);
    }

    #[test]
    fn option_builder_default_test() {
        let dataset_options = DatasetOptions {
            scaling: (1024, 1024),
            red_band_index: 1,
            green_band_index: 2,
            blue_band_index: 3,
        };

        let dataset_options_from_builder: DatasetOptions = DatasetOptionsBuilder::new().build();

        assert_eq!(dataset_options, dataset_options_from_builder);
    }

    #[test]
    fn get_elevation() {
        let mountain_x = 9.68505;
        let mountain_y = 56.105169;
        let mountain_height = 147.0;

        let temp_dir = tempdir().unwrap();
        let temp_dir_path = temp_dir.into_path();

        let mut current_dir = env::current_dir().expect("Current directory not set");
        current_dir.pop();

        let mut elevation_path = current_dir.clone();
        elevation_path.push("resources/test/Geotiff/Elevation_test/elevation");

        let mut dataset_path = current_dir.clone();
        dataset_path.push("resources/test/Geotiff/Elevation_test/map_data");

        let map_ds = dataset_from_folder(&dataset_path.to_str().unwrap()).unwrap();
        let elevation_ds = dataset_from_folder(&elevation_path.to_str().unwrap()).unwrap();

        let mut vrt_path = temp_dir_path.clone();
        vrt_path.push("map.vrt");
        let mut elevation_vrt_path = temp_dir_path.clone();
        elevation_vrt_path.push("elevation.vrt");

        let ds_vrt = build_vrt(Some(vrt_path.as_path()), &map_ds, None).unwrap();

        let elevation_vrt = build_vrt(Some(elevation_vrt_path.as_path()), &elevation_ds, None).unwrap();

        let options = DatasetOptions::builder().build();

        let mosaic = MosaicedDataset { dataset: ds_vrt, options, min_max: None, elevation: Some(elevation_vrt) };

        let coordinates = mosaic.get_world_coordinates(8220.6, 12000.0 - 1262.028);

        assert!(coordinates.is_ok());
        assert!((coordinates.as_ref().unwrap().0 - mountain_x).abs() <= f32::EPSILON.into());
        assert!((coordinates.as_ref().unwrap().1 - mountain_y).abs() <= f32::EPSILON.into());
        assert!((coordinates.as_ref().unwrap().2 - mountain_height).abs() <= 2.0);
    }

    #[test]
    fn elevation_vrt_creation() {
        let temp_dir = tempdir().unwrap();
        let temp_dir_path = temp_dir.into_path();

        let mut current_dir = env::current_dir().expect("Current directory not set");
        current_dir.pop();

        let mut elevation_path = current_dir.clone();
        elevation_path.push("resources/test/Geotiff/Elevation_test/elevation");

        let mut dataset_path = current_dir.clone();
        dataset_path.push("resources/test/Geotiff/Elevation_test/map_data");

        let map_ds = dataset_from_folder(&dataset_path.to_str().unwrap()).unwrap();

        let mut vrt_path = temp_dir_path.clone();
        vrt_path.push("map.vrt");

        let ds_vrt = build_vrt(Some(vrt_path.as_path()), &map_ds, None).unwrap();

        let options = DatasetOptions::builder().build();

        let mut mosaic = MosaicedDataset { dataset: ds_vrt, options, min_max: None, elevation: None };

        let elevation_vrt_path = temp_dir_path.clone();

        let result = mosaic.set_elevation_dataset(elevation_path.to_str().unwrap(), elevation_vrt_path.to_str().unwrap());

        assert!(result.is_ok());

        assert!(mosaic.elevation.is_some());

        assert_eq!(mosaic.elevation.unwrap().raster_count(), 1);

    }
}
