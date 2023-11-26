use crate::error::Error;
use std::mem::MaybeUninit;
use windows::{
    core::ComInterface,
    Win32::Graphics::{
        Direct3D::{self, D3D_DRIVER_TYPE_HARDWARE},
        Direct3D11::{
            self, D3D11CreateDevice, ID3D11Device, ID3D11Multithread, ID3D11Texture2D,
            D3D11_SDK_VERSION,
        },
        Dxgi::{
            Common::DXGI_FORMAT, IDXGIDevice, IDXGIOutput, IDXGIOutput1, IDXGIOutput5,
            IDXGIOutputDuplication, DXGI_ERROR_ACCESS_LOST, DXGI_ERROR_WAIT_TIMEOUT,
            DXGI_OUTDUPL_DESC,
        },
    },
};

pub struct DisplayDuplicatorImpl {
    output_duplication: IDXGIOutputDuplication,
}

/// SAFETY: This is just a COM pointer
unsafe impl Send for DisplayDuplicatorImpl {}

impl DisplayDuplicatorImpl {
    pub fn new(
        d3d11_device: ID3D11Device,
        display_index: u32,
        supported_formats: &[DXGI_FORMAT],
    ) -> Result<Self, Error> {
        let dxgi_device: IDXGIDevice = d3d11_device.cast()?;

        let dxgi_output = unsafe {
            let adapter = dxgi_device.GetAdapter()?;
            adapter.EnumOutputs(display_index)?
        };

        let inner = match DisplayDuplicatorImpl::hdr_duplicator(
            &dxgi_output,
            &dxgi_device,
            supported_formats,
        ) {
            Ok(duplicator) => duplicator,
            Err(e) => {
                tracing::error!("DisplayDuplicator::hdr_duplicator error: {}", e);

                // Fallback to IDXGIOutput1 duplication
                DisplayDuplicatorImpl::sdr_duplicator(&dxgi_output, &dxgi_device)?
            }
        };

        Ok(Self {
            output_duplication: inner,
        })
    }

    fn hdr_duplicator(
        dxgi_output: &IDXGIOutput,
        dxgi_device: &IDXGIDevice,
        supported_formats: &[DXGI_FORMAT],
    ) -> Result<IDXGIOutputDuplication, windows::core::Error> {
        const RESERVED_FLAG: u32 = 0;
        let hdr_output: IDXGIOutput5 = dxgi_output.cast()?;
        unsafe { hdr_output.DuplicateOutput1(dxgi_device, RESERVED_FLAG, supported_formats) }
    }

    fn sdr_duplicator(
        dxgi_output: &IDXGIOutput,
        dxgi_device: &IDXGIDevice,
    ) -> Result<IDXGIOutputDuplication, windows::core::Error> {
        let sdr_output: IDXGIOutput1 = dxgi_output.cast()?;
        unsafe { sdr_output.DuplicateOutput(dxgi_device) }
    }

    /// Returns a description of the display that is currently being duplicated.
    pub fn desc(&self) -> DXGI_OUTDUPL_DESC {
        let mut dupl_desc = MaybeUninit::uninit();
        unsafe {
            // NOTE: `GetDesc` always succeeds if the `IDXGIOutputDuplication` used is valid
            self.output_duplication.GetDesc(dupl_desc.as_mut_ptr());
            dupl_desc.assume_init()
        }
    }

    /// Get the next available frame.
    ///
    /// This method returns an `AcquiredFrame` on success. An error of value
    /// `AcquireFrameError::Retry` is non-fatal and the caller can try to call this method again.
    #[inline]
    pub fn acquire_frame(&mut self, timeout_millis: u32) -> Result<AcquiredFrame<'_>, Error> {
        let (resource, frame_info) = unsafe {
            let mut frame_info = MaybeUninit::uninit();
            let mut resource = MaybeUninit::uninit();

            self.output_duplication.AcquireNextFrame(
                timeout_millis,
                frame_info.as_mut_ptr(),
                resource.as_mut_ptr(),
            )?;

            // SAFETY:
            // Both `image` and `frame_info` are guaranteed to be initialized if
            // `AcquireNextFrame` succeeds.
            (
                resource.assume_init().unwrap_unchecked(),
                frame_info.assume_init(),
            )
        };

        // SAFETY: `IDXGIResource` to `ID3D11Texture2D` never fails
        let image = unsafe { resource.cast().unwrap_unchecked() };

        let acquired_image = AcquiredFrame {
            frame: image,
            parent: self,
            timestamp: frame_info.LastPresentTime,
        };

        Ok(acquired_image)
    }

    /// Create a new D3D11 device.
    // TODO: Move this somewhere else.
    pub fn create_d3d11_device(
        enable_multithreading: bool,
    ) -> Result<ID3D11Device, windows::core::Error> {
        let device = unsafe {
            let driver_type = D3D_DRIVER_TYPE_HARDWARE;
            let flags = Direct3D11::D3D11_CREATE_DEVICE_FLAG(0);
            let feature_levels = [
                Direct3D::D3D_FEATURE_LEVEL_12_2,
                Direct3D::D3D_FEATURE_LEVEL_12_1,
                Direct3D::D3D_FEATURE_LEVEL_12_0,
                Direct3D::D3D_FEATURE_LEVEL_11_1,
                Direct3D::D3D_FEATURE_LEVEL_11_0,
                Direct3D::D3D_FEATURE_LEVEL_10_1,
                Direct3D::D3D_FEATURE_LEVEL_10_0,
                Direct3D::D3D_FEATURE_LEVEL_9_1,
            ];
            let sdk_version = D3D11_SDK_VERSION;

            let mut tmp = MaybeUninit::uninit();

            D3D11CreateDevice(
                None,
                driver_type,
                None,
                flags,
                Some(feature_levels.as_slice()),
                sdk_version,
                Some(tmp.as_mut_ptr()),
                None,
                None,
            )?;

            tmp.assume_init()
                .ok_or_else(|| windows::core::Error::from_win32())?
        };

        // Enabling multithreaded protection might prevent random deadlocks. The performance cost
        // is somewhat negligible.
        unsafe {
            let device_context = device.GetImmediateContext()?;
            let multithreaded: ID3D11Multithread = device_context.cast()?;
            multithreaded.SetMultithreadProtected(enable_multithreading);
        }

        Ok(device)
    }
}

/// Result of a successful `DisplayDuplicator::acquire_frame`.
pub struct AcquiredFrame<'a> {
    frame: ID3D11Texture2D,
    parent: &'a mut DisplayDuplicatorImpl,
    timestamp: i64,
}

impl<'a> Drop for AcquiredFrame<'a> {
    #[inline]
    fn drop(&mut self) {
        unsafe {
            let _ = self.parent.output_duplication.ReleaseFrame();
        }
    }
}

impl<'a> AsRef<ID3D11Texture2D> for AcquiredFrame<'a> {
    fn as_ref(&self) -> &ID3D11Texture2D {
        &self.frame
    }
}

impl<'a> AcquiredFrame<'a> {
    #[inline]
    pub fn timestamp(&self) -> i64 {
        self.timestamp
    }
}

impl From<windows::core::Error> for Error {
    fn from(value: windows::core::Error) -> Self {
        tracing::error!("{}", value);

        match value.code() {
            DXGI_ERROR_WAIT_TIMEOUT => Error::WaitTimeout,
            DXGI_ERROR_ACCESS_LOST => Error::AccessLost,
            _ => Error::InternalError,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use windows::Win32::Graphics::Dxgi::Common::{
        DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_FORMAT_R10G10B10A2_UNORM, DXGI_FORMAT_R8G8B8A8_UNORM,
    };

    #[test]
    fn display_duplicator_desc() {
        let device = DisplayDuplicatorImpl::create_d3d11_device(true).unwrap();
        let supported_formats = vec![
            DXGI_FORMAT_B8G8R8A8_UNORM,
            DXGI_FORMAT_R10G10B10A2_UNORM,
            DXGI_FORMAT_R8G8B8A8_UNORM,
        ];
        let duplicator = DisplayDuplicatorImpl::new(device, 0, &supported_formats).unwrap();
        let desc = duplicator.desc();
        dbg!(desc);
    }
}
