use std::path::{Path, PathBuf};

use ashpd::desktop::screenshot::Screenshot;
use image::DynamicImage;

use crate::{
    CaptureProvider, CaptureRegion, CaptureRequest, CapturedFrame, OcrError, Rect, Result,
};

#[derive(Clone, Debug, Default)]
pub struct PortalScreenshotCaptureProvider;

impl CaptureProvider for PortalScreenshotCaptureProvider {
    fn capture(&self, request: CaptureRequest) -> Result<CapturedFrame> {
        log::debug!(
            "capturing frame through ashpd screenshot portal: monitor={:?}, region={:?}",
            request.monitor,
            request.region
        );

        let path = async_io::block_on(request_screenshot_path())?;
        log::debug!(
            "ashpd screenshot portal returned image at {}",
            path.display()
        );

        let image = image::open(&path).map_err(|err| {
            OcrError::ImageProcessing(format!(
                "could not load ashpd screenshot portal image {}: {err}",
                path.display()
            ))
        })?;
        let image = crop_to_requested_region(image, request.region)?;
        let frame = CapturedFrame::new(image);

        log::debug!("captured frame: {}x{}", frame.width(), frame.height());

        Ok(frame)
    }
}

fn crop_to_requested_region(
    image: DynamicImage,
    region: Option<CaptureRegion>,
) -> Result<DynamicImage> {
    let Some(region) = region else {
        return Ok(image);
    };
    let region = scale_region_to_screenshot_pixels(region, image.width(), image.height())?;

    if region.width == 0 || region.height == 0 {
        return Err(OcrError::Capture(format!(
            "configured capture region has an empty size: {region:?}"
        )));
    }

    if region.right() > image.width() || region.bottom() > image.height() {
        return Err(OcrError::Capture(format!(
            "configured capture region {region:?} is outside screenshot {}x{}",
            image.width(),
            image.height()
        )));
    }

    log::debug!(
        "cropping portal screenshot {}x{} to configured monitor region {:?}",
        image.width(),
        image.height(),
        region
    );

    Ok(image.crop_imm(region.x, region.y, region.width, region.height))
}

fn scale_region_to_screenshot_pixels(
    region: CaptureRegion,
    screenshot_width: u32,
    screenshot_height: u32,
) -> Result<Rect> {
    if region.desktop_bounds.width == 0 || region.desktop_bounds.height == 0 {
        return Err(OcrError::Capture(format!(
            "configured logical desktop bounds have an empty size: {:?}",
            region.desktop_bounds
        )));
    }

    if region.region.x < region.desktop_bounds.x
        || region.region.y < region.desktop_bounds.y
        || region.region.right() > region.desktop_bounds.right()
        || region.region.bottom() > region.desktop_bounds.bottom()
    {
        return Err(OcrError::Capture(format!(
            "configured capture region {:?} is outside logical desktop bounds {:?}",
            region.region, region.desktop_bounds
        )));
    }

    let left = scale_floor(
        region.region.x - region.desktop_bounds.x,
        screenshot_width,
        region.desktop_bounds.width,
    );
    let top = scale_floor(
        region.region.y - region.desktop_bounds.y,
        screenshot_height,
        region.desktop_bounds.height,
    );
    let right = scale_ceil(
        region.region.right() - region.desktop_bounds.x,
        screenshot_width,
        region.desktop_bounds.width,
    );
    let bottom = scale_ceil(
        region.region.bottom() - region.desktop_bounds.y,
        screenshot_height,
        region.desktop_bounds.height,
    );

    let scaled = Rect {
        x: left,
        y: top,
        width: right.saturating_sub(left),
        height: bottom.saturating_sub(top),
    };

    log::debug!(
        "scaled logical capture region {:?} within desktop {:?} to screenshot pixel region {:?} for screenshot {}x{}",
        region.region,
        region.desktop_bounds,
        scaled,
        screenshot_width,
        screenshot_height
    );

    Ok(scaled)
}

fn scale_floor(value: u32, target: u32, source: u32) -> u32 {
    ((u64::from(value) * u64::from(target)) / u64::from(source)) as u32
}

fn scale_ceil(value: u32, target: u32, source: u32) -> u32 {
    ((u64::from(value) * u64::from(target)).div_ceil(u64::from(source))) as u32
}

async fn request_screenshot_path() -> Result<PathBuf> {
    let response = Screenshot::request()
        .interactive(false)
        .modal(false)
        .send()
        .await
        .map_err(portal_error)?
        .response()
        .map_err(portal_error)?;

    screenshot_uri_to_path(response.uri().as_str())
}

fn portal_error(err: ashpd::Error) -> OcrError {
    OcrError::Capture(err.to_string())
}

fn screenshot_uri_to_path(uri: &str) -> Result<PathBuf> {
    let path = uri.strip_prefix("file://").ok_or_else(|| {
        OcrError::Capture(format!("screenshot portal returned non-file URI {uri:?}"))
    })?;

    let decoded = percent_decode_file_uri_path(path)?;
    Ok(Path::new(&decoded).to_path_buf())
}

fn percent_decode_file_uri_path(path: &str) -> Result<String> {
    let bytes = path.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;

    while index < bytes.len() {
        if bytes[index] == b'%' {
            let high = bytes.get(index + 1).copied().ok_or_else(|| {
                OcrError::Capture(format!(
                    "invalid percent-encoded screenshot URI path {path:?}"
                ))
            })?;
            let low = bytes.get(index + 2).copied().ok_or_else(|| {
                OcrError::Capture(format!(
                    "invalid percent-encoded screenshot URI path {path:?}"
                ))
            })?;
            let high = hex_value(high).ok_or_else(|| {
                OcrError::Capture(format!(
                    "invalid percent-encoded screenshot URI path {path:?}"
                ))
            })?;
            let low = hex_value(low).ok_or_else(|| {
                OcrError::Capture(format!(
                    "invalid percent-encoded screenshot URI path {path:?}"
                ))
            })?;
            decoded.push((high << 4) | low);
            index += 3;
        } else {
            decoded.push(bytes[index]);
            index += 1;
        }
    }

    String::from_utf8(decoded)
        .map_err(|err| OcrError::Capture(format!("screenshot URI path is not UTF-8: {err}")))
}

fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        crop_to_requested_region, percent_decode_file_uri_path, scale_region_to_screenshot_pixels,
        screenshot_uri_to_path,
    };
    use crate::{CaptureRegion, Rect};
    use image::{DynamicImage, RgbaImage};

    #[test]
    fn screenshot_file_uri_is_converted_to_path() {
        let path = screenshot_uri_to_path("file:///tmp/wf-info%20capture.png").expect("path");

        assert_eq!(path.to_string_lossy(), "/tmp/wf-info capture.png");
    }

    #[test]
    fn non_file_screenshot_uri_is_rejected() {
        let err = screenshot_uri_to_path("https://example.test/capture.png").expect_err("file URI");

        assert!(err.to_string().contains("non-file URI"));
    }

    #[test]
    fn invalid_percent_encoding_is_rejected() {
        let err = percent_decode_file_uri_path("/tmp/%xx.png").expect_err("invalid percent");

        assert!(err.to_string().contains("percent-encoded"));
    }

    #[test]
    fn portal_screenshot_can_be_cropped_to_monitor_region() {
        let image = DynamicImage::ImageRgba8(RgbaImage::new(200, 100));
        let cropped = crop_to_requested_region(
            image,
            Some(CaptureRegion {
                region: Rect {
                    x: 50,
                    y: 10,
                    width: 80,
                    height: 60,
                },
                desktop_bounds: Rect {
                    x: 0,
                    y: 0,
                    width: 200,
                    height: 100,
                },
            }),
        )
        .expect("cropped frame");

        assert_eq!(cropped.width(), 80);
        assert_eq!(cropped.height(), 60);
    }

    #[test]
    fn capture_region_must_fit_inside_logical_desktop_bounds() {
        let image = DynamicImage::ImageRgba8(RgbaImage::new(200, 100));
        let err = crop_to_requested_region(
            image,
            Some(CaptureRegion {
                region: Rect {
                    x: 150,
                    y: 10,
                    width: 80,
                    height: 60,
                },
                desktop_bounds: Rect {
                    x: 0,
                    y: 0,
                    width: 200,
                    height: 100,
                },
            }),
        )
        .expect_err("out of bounds");

        assert!(err.to_string().contains("outside logical desktop bounds"));
    }

    #[test]
    fn logical_region_is_scaled_to_screenshot_pixels() {
        let scaled = scale_region_to_screenshot_pixels(
            CaptureRegion {
                region: Rect {
                    x: 2560,
                    y: 0,
                    width: 2560,
                    height: 1440,
                },
                desktop_bounds: Rect {
                    x: 0,
                    y: 0,
                    width: 5120,
                    height: 1440,
                },
            },
            7680,
            2160,
        )
        .expect("scaled region");

        assert_eq!(
            scaled,
            Rect {
                x: 3840,
                y: 0,
                width: 3840,
                height: 2160
            }
        );
    }
}
