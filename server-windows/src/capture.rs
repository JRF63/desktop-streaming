use std::mem::MaybeUninit;
use windows::{
    core::Interface,
    Win32::{
        Foundation::E_ACCESSDENIED,
        Graphics::{
            Direct3D11::{ID3D11Device, ID3D11Texture2D},
            Dxgi::{
                Common::DXGI_FORMAT, IDXGIDevice, IDXGIOutput, IDXGIOutput1, IDXGIOutput5,
                IDXGIOutputDuplication, DXGI_ERROR_ACCESS_LOST, DXGI_ERROR_WAIT_TIMEOUT,
                DXGI_OUTDUPL_DESC, DXGI_OUTDUPL_FRAME_INFO,
            },
        },
        UI::HiDpi::{
            GetProcessDpiAwareness, SetProcessDpiAwareness, PROCESS_PER_MONITOR_DPI_AWARE,
            PROCESS_SYSTEM_DPI_AWARE,
        },
    },
};

#[repr(C)]
pub struct ScreenDuplicator {
    /// Interface that does the duplication.
    output_dupl: IDXGIOutputDuplication,
    /// Represents the output of the GPU.
    dxgi_output: IDXGIOutput,
    /// GPU from which the output is being duplicated.
    dxgi_device: IDXGIDevice,
    /// Texture formats that the duplicator can output
    supported_formats: Box<[DXGI_FORMAT]>,
    /// Cached result for the usage of IDXGIOutput5.
    is_dpi_aware: bool,
}

impl Drop for ScreenDuplicator {
    fn drop(&mut self) {
        let _ = self.release_frame();
    }
}

unsafe impl Send for ScreenDuplicator {}

impl ScreenDuplicator {
    /// Creates a new ScreenDuplicator.
    pub fn new(
        d3d11_device: ID3D11Device,
        display_index: u32,
        supported_formats: Vec<DXGI_FORMAT>,
    ) -> Result<ScreenDuplicator, windows::core::Error> {
        let supported_formats = supported_formats.into_boxed_slice();
        let is_dpi_aware = ScreenDuplicator::try_set_dpi_aware()?;
        let dxgi_device: IDXGIDevice = d3d11_device.cast()?;

        // SAFETY: Windows API call
        let dxgi_output = unsafe {
            let adapter = dxgi_device.GetAdapter()?;
            adapter.EnumOutputs(display_index)?
        };

        let output_dupl = ScreenDuplicator::new_output_duplicator(
            &dxgi_output,
            &dxgi_device,
            &supported_formats,
            is_dpi_aware,
        )?;

        Ok(ScreenDuplicator {
            output_dupl,
            dxgi_output,
            dxgi_device,
            supported_formats,
            is_dpi_aware,
        })
    }

    /// Returns a description of the display that is currently being duplicated.
    pub fn desc(&self) -> DXGI_OUTDUPL_DESC {
        let mut dupl_desc: MaybeUninit<DXGI_OUTDUPL_DESC> = MaybeUninit::uninit();
        unsafe {
            // NOTE: `GetDesc` always succeeds if the `IDXGIOutputDuplication` used is valid
            self.output_dupl.GetDesc(dupl_desc.as_mut_ptr());
            dupl_desc.assume_init()
        }
    }

    /// Get the next available frame.
    /// 
    /// This method returns an `AcquiredFrame` on success. An error of value
    /// `AcquireFrameError::Retry` is non-fatal and the caller can try to call this method again.
    #[inline]
    pub fn acquire_frame<'a>(
        &'a mut self,
        timeout_millis: u32,
    ) -> Result<(AcquiredFrame<'a>, DXGI_OUTDUPL_FRAME_INFO), AcquireFrameError> {
        let mut frame_info: MaybeUninit<DXGI_OUTDUPL_FRAME_INFO> = MaybeUninit::uninit();
        let mut resource = None;

        // SAFETY: Windows API call
        let result = unsafe {
            self.output_dupl.AcquireNextFrame(
                timeout_millis,
                frame_info.as_mut_ptr(),
                &mut resource,
            )
        };

        match result {
            Ok(_) => {
                // SAFETY:
                // Both `image` and `frame_info` are guaranteed to be initialized if
                // `AcquireNextFrame` succeeds.
                let (resource, frame_info) =
                    unsafe { (resource.unwrap_unchecked(), frame_info.assume_init()) };

                // SAFETY: `IDXGIResource` to `ID3D11Texture2D` should never fail.
                let image = unsafe { resource.cast().unwrap_unchecked() };

                let acquired_image = AcquiredFrame {
                    frame: image,
                    duplicator: self,
                };

                Ok((acquired_image, frame_info))
            }
            Err(e) => match e.code() {
                DXGI_ERROR_WAIT_TIMEOUT => Err(AcquireFrameError::Retry),
                DXGI_ERROR_ACCESS_LOST => {
                    // Reset duplicator then move on to next frame acquisition
                    self.reset_output_duplicator()
                        .map_err(|_| AcquireFrameError::Unknown)?;
                    Err(AcquireFrameError::Retry)
                }
                _ => Err(AcquireFrameError::Unknown),
            },
        }
    }

    /// Signals that the current frame is done being processed.
    #[inline]
    fn release_frame(&mut self) -> Result<(), windows::core::Error> {
        unsafe {
            self.output_dupl.ReleaseFrame()?;
            Ok(())
        }
    }

    /// Creates a new IDXGIOutputDuplication. Used when access is lost to the display due to
    /// desktop switching or using full-screen programs.
    #[inline]
    pub fn reset_output_duplicator(&mut self) -> Result<(), windows::core::Error> {
        let output_dupl = ScreenDuplicator::new_output_duplicator(
            &self.dxgi_output,
            &self.dxgi_device,
            &self.supported_formats,
            self.is_dpi_aware,
        )?;

        // This also frees the old output_dupl if it's not null
        self.output_dupl = output_dupl;

        Ok(())
    }

    /// Returns a new output duplicator.
    fn new_output_duplicator(
        dxgi_output: &IDXGIOutput,
        dxgi_device: &IDXGIDevice,
        supported_formats: &[DXGI_FORMAT],
        is_dpi_aware: bool,
    ) -> Result<IDXGIOutputDuplication, windows::core::Error> {
        // First test if output_dupl can be made with IDXGIOutput5
        if is_dpi_aware {
            unsafe {
                let hdr_output: IDXGIOutput5 = dxgi_output.cast()?;

                const RESERVED_FLAG: u32 = 0;
                let output_dupl =
                    hdr_output.DuplicateOutput1(dxgi_device, RESERVED_FLAG, supported_formats);
                if output_dupl.is_ok() {
                    return output_dupl;
                }
            }
        }

        // If either not DPI aware or IDXGIOutput5 failed, fall back to IDXGIOutput1
        unsafe {
            let sdr_output: IDXGIOutput1 = dxgi_output.cast()?;
            sdr_output.DuplicateOutput(dxgi_device)
        }
    }

    /// Attempt to signal to the OS that the process is DPI aware.
    /// Returns the DPI awareness.
    fn try_set_dpi_aware() -> Result<bool, windows::core::Error> {
        unsafe {
            if let Err(e) = SetProcessDpiAwareness(PROCESS_PER_MONITOR_DPI_AWARE) {
                // `E_ACCESSDENIED` means the DPI awareness has already been set
                if e.code() != E_ACCESSDENIED {
                    return Err(e);
                }
            }
            let awareness = GetProcessDpiAwareness(None)?;
            if awareness == PROCESS_SYSTEM_DPI_AWARE || awareness == PROCESS_PER_MONITOR_DPI_AWARE {
                Ok(true)
            } else {
                Ok(false)
            }
        }
    }
}

/// Result of a successful `ScreenDuplicator::acquire_frame`.
pub struct AcquiredFrame<'a> {
    frame: ID3D11Texture2D,
    duplicator: &'a mut ScreenDuplicator,
}

impl<'a> Drop for AcquiredFrame<'a> {
    #[inline]
    fn drop(&mut self) {
        let _ = self.duplicator.release_frame();
    }
}

impl<'a> AsRef<ID3D11Texture2D> for AcquiredFrame<'a> {
    fn as_ref(&self) -> &ID3D11Texture2D {
        &self.frame
    }
}

/// Errors that `ScreenDuplicator::acquire_frame` can return.
#[derive(Debug)]
pub enum AcquireFrameError {
    Retry,
    Unknown,
}

#[cfg(test)]
mod tests {
    use super::*;
    use windows::Win32::Graphics::Dxgi::Common::{
        DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_FORMAT_R10G10B10A2_UNORM, DXGI_FORMAT_R8G8B8A8_UNORM,
    };

    #[test]
    fn duplicator_desc() {
        let device = crate::device::create_d3d11_device().unwrap();
        let duplicator = ScreenDuplicator::new(
            device,
            0,
            vec![
                DXGI_FORMAT_B8G8R8A8_UNORM,
                DXGI_FORMAT_R10G10B10A2_UNORM,
                DXGI_FORMAT_R8G8B8A8_UNORM,
            ],
        )
        .unwrap();
        let desc = duplicator.desc();
        dbg!(desc);
    }

    #[test]
    fn refresh_rate_test() {
        use std::time::Duration;

        let x = Duration::from_secs(1000 as u64);
        let y = x / 74973;
        println!("{y:?}");
    }
}
