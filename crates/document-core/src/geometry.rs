use crate::CoreError;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PdfPoint {
    pub x: f64,
    pub y: f64,
}

impl PdfPoint {
    pub const fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ViewportPoint {
    pub x: f64,
    pub y: f64,
}

impl ViewportPoint {
    pub const fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PdfRect {
    pub x_min: f64,
    pub y_min: f64,
    pub x_max: f64,
    pub y_max: f64,
}

impl PdfRect {
    pub fn new(x_min: f64, y_min: f64, x_max: f64, y_max: f64) -> Result<Self, CoreError> {
        let values = [x_min, y_min, x_max, y_max];
        if !values.iter().all(|value| value.is_finite()) {
            return Err(CoreError::invalid_geometry("page rectangle must be finite"));
        }
        if x_max <= x_min || y_max <= y_min {
            return Err(CoreError::invalid_geometry(
                "page rectangle must have positive area",
            ));
        }
        Ok(Self {
            x_min,
            y_min,
            x_max,
            y_max,
        })
    }

    pub fn width(self) -> f64 {
        self.x_max - self.x_min
    }

    pub fn height(self) -> f64 {
        self.y_max - self.y_min
    }

    pub fn intersection(self, other: Self) -> Option<Self> {
        Self::new(
            self.x_min.max(other.x_min),
            self.y_min.max(other.y_min),
            self.x_max.min(other.x_max),
            self.y_max.min(other.y_max),
        )
        .ok()
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Rotation {
    #[default]
    None,
    Clockwise90,
    Clockwise180,
    Clockwise270,
}

impl Rotation {
    pub fn from_degrees(degrees: i32) -> Result<Self, CoreError> {
        match degrees.rem_euclid(360) {
            0 => Ok(Self::None),
            90 => Ok(Self::Clockwise90),
            180 => Ok(Self::Clockwise180),
            270 => Ok(Self::Clockwise270),
            _ => Err(CoreError::invalid_geometry(
                "page rotation must be a multiple of 90 degrees",
            )),
        }
    }

    pub const fn degrees(self) -> i32 {
        match self {
            Self::None => 0,
            Self::Clockwise90 => 90,
            Self::Clockwise180 => 180,
            Self::Clockwise270 => 270,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PageGeometry {
    pub media_box: PdfRect,
    pub crop_box: PdfRect,
    pub rotation: Rotation,
    pub user_unit: f64,
}

impl PageGeometry {
    pub fn new(
        media_box: PdfRect,
        crop_box: PdfRect,
        rotation: Rotation,
        user_unit: f64,
    ) -> Result<Self, CoreError> {
        if !user_unit.is_finite() || user_unit <= 0.0 {
            return Err(CoreError::invalid_geometry(
                "user unit must be finite and positive",
            ));
        }
        let crop_box = media_box.intersection(crop_box).unwrap_or(media_box);
        Ok(Self {
            media_box,
            crop_box,
            rotation,
            user_unit,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ViewportTransform {
    crop_box: PdfRect,
    rotation: Rotation,
    scale: f64,
}

impl ViewportTransform {
    pub fn new(geometry: PageGeometry, zoom: f64, device_scale: f64) -> Result<Self, CoreError> {
        if !zoom.is_finite() || zoom <= 0.0 || !device_scale.is_finite() || device_scale <= 0.0 {
            return Err(CoreError::invalid_geometry(
                "viewport scales must be finite and positive",
            ));
        }
        Ok(Self {
            crop_box: geometry.crop_box,
            rotation: geometry.rotation,
            scale: zoom * device_scale * geometry.user_unit,
        })
    }

    pub fn viewport_size(self) -> (f64, f64) {
        let width = self.crop_box.width() * self.scale;
        let height = self.crop_box.height() * self.scale;
        match self.rotation {
            Rotation::None | Rotation::Clockwise180 => (width, height),
            Rotation::Clockwise90 | Rotation::Clockwise270 => (height, width),
        }
    }

    pub fn pdf_to_viewport(self, point: PdfPoint) -> ViewportPoint {
        let x = point.x - self.crop_box.x_min;
        let y = point.y - self.crop_box.y_min;
        let width = self.crop_box.width();
        let height = self.crop_box.height();
        let (x, y) = match self.rotation {
            Rotation::None => (x, height - y),
            Rotation::Clockwise90 => (y, x),
            Rotation::Clockwise180 => (width - x, y),
            Rotation::Clockwise270 => (height - y, width - x),
        };
        ViewportPoint::new(x * self.scale, y * self.scale)
    }

    pub fn viewport_to_pdf(self, point: ViewportPoint) -> PdfPoint {
        let x = point.x / self.scale;
        let y = point.y / self.scale;
        let width = self.crop_box.width();
        let height = self.crop_box.height();
        let (x, y) = match self.rotation {
            Rotation::None => (x, height - y),
            Rotation::Clockwise90 => (y, x),
            Rotation::Clockwise180 => (width - x, y),
            Rotation::Clockwise270 => (width - y, height - x),
        };
        PdfPoint::new(x + self.crop_box.x_min, y + self.crop_box.y_min)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn geometry(rotation: Rotation) -> PageGeometry {
        let media = PdfRect::new(10.0, 20.0, 210.0, 120.0).unwrap();
        PageGeometry::new(media, media, rotation, 1.0).unwrap()
    }

    #[test]
    fn crop_box_is_clamped_to_media_box() {
        let media = PdfRect::new(0.0, 0.0, 100.0, 100.0).unwrap();
        let crop = PdfRect::new(50.0, -20.0, 140.0, 80.0).unwrap();
        let geometry = PageGeometry::new(media, crop, Rotation::None, 1.0).unwrap();

        assert_eq!(
            geometry.crop_box,
            PdfRect::new(50.0, 0.0, 100.0, 80.0).unwrap()
        );
    }

    #[test]
    fn rotations_produce_expected_viewport_sizes() {
        let normal = ViewportTransform::new(geometry(Rotation::None), 2.0, 1.5).unwrap();
        let rotated = ViewportTransform::new(geometry(Rotation::Clockwise90), 2.0, 1.5).unwrap();

        assert_eq!(normal.viewport_size(), (600.0, 300.0));
        assert_eq!(rotated.viewport_size(), (300.0, 600.0));
    }

    #[test]
    fn every_rotation_round_trips_points() {
        for rotation in [
            Rotation::None,
            Rotation::Clockwise90,
            Rotation::Clockwise180,
            Rotation::Clockwise270,
        ] {
            let transform = ViewportTransform::new(geometry(rotation), 1.25, 2.0).unwrap();
            let pdf = PdfPoint::new(60.0, 70.0);
            let round_trip = transform.viewport_to_pdf(transform.pdf_to_viewport(pdf));

            assert!((round_trip.x - pdf.x).abs() < f64::EPSILON);
            assert!((round_trip.y - pdf.y).abs() < f64::EPSILON);
        }
    }

    #[test]
    fn malformed_rotation_is_rejected() {
        assert!(Rotation::from_degrees(45).is_err());
    }
}
