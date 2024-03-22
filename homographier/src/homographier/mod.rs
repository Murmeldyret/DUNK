use std::marker::PhantomData;

use opencv::{
    calib3d::{find_homography, prelude::*, RANSAC},
    core::{InputArray, Scalar, Size, ToInputArray, ToOutputArray, Vec4b, CV_8UC4},
    imgcodecs::{ImreadModes, IMREAD_COLOR},
    prelude::*,
};
use rgb::*;

pub trait PixelElemType {
    fn to_cv_const(&self) -> i32;
}
pub struct BGRA;
impl PixelElemType for BGRA {
    fn to_cv_const(&self) -> i32 {
        CV_8UC4
    }
}

#[non_exhaustive]
#[derive(Debug)]
pub enum MatError {
    /// Inner openCV errors
    Opencv(opencv::Error),
    /// The mat is empty, and not considered valid
    Empty,
    /// Matrix is not rectangular (columns or rows with differing lengths)
    Jagged,
    /// An unknown error
    Unknown,
}

/// Checked Mat type
/// # Notes
/// Guarantees that a contained mat contains data, but makes no assumptions about validity
#[derive(Debug)]
pub struct Cmat<T> {
    mat: Mat,
    _marker: PhantomData<T>,
}

impl<T> Cmat<T> {
    pub fn new(mat: Mat) -> Result<Self, MatError> {
        Cmat {
            mat,
            _marker: PhantomData,
        }
        .check_owned()
    }

    pub fn imread_checked(filename: &str, flags: i32) -> Result<Self, MatError> {
        // let res =
        Cmat::new(opencv::imgcodecs::imread(filename, flags).map_err(MatError::Opencv)?)
    }

    fn check_owned(self) -> Result<Self, MatError> {
        match self.mat.dims() {
            // dims will always be >=2, unless the Mat is empty
            0 => Err(MatError::Empty),
            _ => Ok(self),
        }
    }

    /// Creates a Cmat from a copied 1-dimensional slice
    pub fn from_2d_slice(slice: &[impl AsRef<[T]>]) -> Result<Self, MatError>
    where
        T: DataType,
    {
        let mat = Mat::from_slice_2d::<T>(slice).map_err(MatError::Opencv)?;
        Cmat::new(mat)
    }

    fn check(&self) -> Result<Self, MatError> {
        unimplemented!("er muligvis ikke nødvendig")
    }

    //further checked functions go here
}

impl<T> ToInputArray for Cmat<T> {
    fn input_array(&self) -> opencv::Result<opencv::core::_InputArray> {
        let res = self.check().map_err(|err| match err {
            MatError::Opencv(inner) => inner,
            _ => opencv::Error {
                code: -2,
                message: "unknown error".into(),
            },
        })?;
        res.input_array()
    }
}
impl<T> ToOutputArray for Cmat<T> {
    fn output_array(&mut self) -> opencv::Result<opencv::core::_OutputArray> {
        self.check()
            .map_err(|err| match err {
                MatError::Opencv(inner) => inner,
                _ => opencv::Error {
                    code: -2,
                    message: "unknown error".into(),
                },
            })?
            .output_array()
    }
}

pub fn raster_to_mat(pixels: &[RGBA8], w: i32, h: i32) -> Result<Cmat<Vec4b>, MatError> {
    //RGBA<u8> is equivalent to opencv's Vec4b, which implements DataType
    if pixels.len() != (w * h) as usize {
        return Err(MatError::Unknown);
    }

    let rows = raster_1d_to_2d(pixels, w, None).map_err(|_err| MatError::Jagged)?;

    let converted: Vec<Vec<Vec4b>> = rows
        .into_iter()
        .map(|row| row.into_iter().map(|p| rbga8_to_vec4b(p)).collect())
        .collect();

    Cmat::from_2d_slice(&converted)
}

fn raster_1d_to_2d(
    pixels: &[RGBA8],
    w: i32,
    vec: Option<Vec<Vec<RGBA8>>>,
) -> Result<Vec<Vec<RGBA8>>, ()> {
    let (first_row, rest) = pixels.split_at(w as usize);
    let mut vec = vec.unwrap_or(Vec::new());

    vec.push(first_row.to_vec());

    let len = pixels.len() % (0 as usize);

    match rest.len() {
        0 => Ok(vec),
        _ if len == 0 => raster_1d_to_2d(pixels, w, Some(vec)),
        _ => Err(()), // if there is not enough pixels to fill a row
    }
    // todo!()
}

fn rbga8_to_vec4b(pixel: RGBA8) -> Vec4b {
    Vec4b::new(pixel.b, pixel.g, pixel.r, pixel.a)
    // todo!()
}

pub fn find_homography_mat(
    input: &impl ToInputArray,
    reference: &impl ToInputArray,
    reproj_threshold: Option<f64>,
) -> Result<Mat, opencv::Error> {
    let mut mask = Mat::default();
    let homography = find_homography(
        input,
        reference,
        &mut mask,
        RANSAC,
        reproj_threshold.unwrap_or(10.0),
    ); // RANSAC is used since some feature matching may be erroneous.

    // homography
    todo!()
}

#[allow(clippy::unwrap_used)]
#[allow(unused_variables)]
mod test {
    use crate::homographier::*;
    use opencv::{
        core::*,
        imgcodecs::{ImreadModes, IMREAD_UNCHANGED},
    };
    use rgb::alt::BGRA8;
    use std::{env, io, path::PathBuf};

    type Image<T> = Vec<Vec<T>>;
    fn path_to_test_images() -> io::Result<PathBuf> {
        let mut img_dir = env::current_dir()?;

        img_dir.pop();
        img_dir.push("resources/test/images");
        Ok(img_dir)
    }

    #[ignore = "Skal bruge Akaze keypoints"]
    #[test]
    fn homography_success() {
        let mut img_dir = path_to_test_images().expect("epic fail");
        img_dir.pop();
        img_dir.push("images");
        // dbg!(current_dir);

        let mut input_path = img_dir.clone();
        input_path.push("3.png");
        let mut reference_path = img_dir.clone();
        reference_path.push("1.png");

        let input = opencv::imgcodecs::imread(
            input_path.to_str().unwrap(),
            ImreadModes::IMREAD_UNCHANGED.into(),
        )
        .unwrap();
        let reference = opencv::imgcodecs::imread(
            reference_path.to_str().unwrap(),
            ImreadModes::IMREAD_UNCHANGED.into(),
        )
        .unwrap();
        // dbg!(&input);
        // dbg!(&reference);
        let res = find_homography_mat(&input, &reference, None);
        let res = res.inspect_err(|e| {
            // dbg!(e);
        });
        assert!(res.is_ok())
    }

    #[test]
    fn cmat_init() {
        assert!(Cmat::<BGRA>::new(Mat::default()).is_err())
    }
    #[test]
    fn cmat_init_2d() {
        let cmat = Cmat::<BGRA>::new(
            Mat::new_size_with_default(Size::new(10, 10), CV_8UC4, Scalar::default()).unwrap(),
        )
        .unwrap();

        assert!(cmat.mat.dims() == 2)
    }

    #[test]
    #[ignore]
    fn mat_ones() {
        let mat: Mat = Mat::ones(2, 2, CV_8UC4).unwrap().to_mat().unwrap();
        // dbg!(mat.at_2d::<i32>(1, 1).unwrap());
        // assert_eq!(*mat.at_3d::<Vec4b>(0, 0, 0).unwrap(),1)
        // let bgramat: Mat4b = Mat4b::try_from(mat).unwrap();
        // let pixels = bgramat.at_2d::<Vec4b>(1, 1).unwrap();
        // pixels.as_bgra();
        // mat.at_2d::<Vec4b>(1, 1).unwrap()
    }

    #[test]
    fn image_correct_pixels() {
        let mut img_dir = path_to_test_images().expect("epic fail");
        img_dir.pop();
        img_dir.push("images");

        img_dir.push("1.png");
        let img = Cmat::<BGRA>::imread_checked(img_dir.to_str().unwrap(), IMREAD_UNCHANGED.into())
            .expect("could not find image at location");

        assert_eq!(img.mat.depth(), CV_8U);
        assert_eq!(img.mat.channels(), 4);
    }

    #[test]
    fn cmat_from_slice() {
        let pixel: Vec4b = Vec4b::new(1, 2, 3, 4);
        let mut image: Image<Vec4b> = Vec::new();
        const IMG_SIZE: usize = 4;

        //matrix init
        for i in 0..IMG_SIZE {
            image.insert(i, Vec::new());
            image[i].reserve(IMG_SIZE);

            for j in 0..IMG_SIZE {
                let scalar: u8 = 1 + j as u8;
                image[i].insert(
                    j,
                    pixel
                        .clone()
                        .mul(Vec4b::new(scalar, scalar, scalar, scalar)),
                );
            }
        }
        let cmat = Cmat::from_2d_slice(&image).unwrap();
        let first_pixel = Vec4b::new(1, 2, 3, 4);
        let sixteenth_pixel = first_pixel.mul(Vec4b::new(4, 4, 4, 4));

        // asserts that pixels are stored row major i.e.
        // [[<1,2,3,4>,<2,4,6,8>,<3,6,9,12>,<4,8,12,16>],
        // [[<1,2,3,4>,<2,4,6,8>,<3,6,9,12>,<4,8,12,16>],
        // [[<1,2,3,4>,<2,4,6,8>,<3,6,9,12>,<4,8,12,16>],
        // [[<1,2,3,4>,<2,4,6,8>,<3,6,9,12>,<4,8,12,16>]]

        assert_eq!(cmat.mat.at_2d::<Vec4b>(0, 0).unwrap().clone(), first_pixel);
        assert_eq!(
            cmat.mat
                .at_2d::<Vec4b>((IMG_SIZE as i32) - 1, (IMG_SIZE as i32) - 1)
                .unwrap()
                .clone(),
            sixteenth_pixel
        );
    }
}