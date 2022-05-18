use std::mem::MaybeUninit;
use windows::{
    core::{Interface, Result},
    Win32::{
        Graphics::{
            Direct3D11::ID3D11Device,
            Dxgi::{
                Common::DXGI_FORMAT, IDXGIDevice, IDXGIOutput, IDXGIOutput1, IDXGIOutput5,
                IDXGIOutputDuplication, IDXGIResource, DXGI_ERROR_ACCESS_LOST,
                DXGI_ERROR_WAIT_TIMEOUT, DXGI_OUTDUPL_DESC, DXGI_OUTDUPL_FRAME_INFO,
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
    /// Timeout for acquiring frame in milliseconds.
    timeout: u32,
    /// Represents the output of the GPU.
    dxgi_output: IDXGIOutput,
    /// GPU from which the output is being duplicated.
    dxgi_device: IDXGIDevice,
    /// Cached result for the usage of IDXGIOutput5.
    dpi_aware: bool,
}

impl ScreenDuplicator {
    /// Creates a new ScreenDuplicator.
    pub fn new(
        d3d11_device: ID3D11Device,
        display_index: u32,
        supported_formats: &[DXGI_FORMAT],
    ) -> Result<ScreenDuplicator> {
        let dpi_aware = ScreenDuplicator::try_set_dpi_aware()?;
        let dxgi_device: IDXGIDevice = d3d11_device.cast()?;
        let duplicator = unsafe {
            let adapter = dxgi_device.GetAdapter()?;
            let dxgi_output = adapter.EnumOutputs(display_index)?;

            let output_dupl = ScreenDuplicator::new_output_duplicator(
                &dxgi_output,
                &dxgi_device,
                supported_formats,
                dpi_aware,
            )?;

            let timeout = ScreenDuplicator::compute_default_timeout(&output_dupl);

            ScreenDuplicator {
                output_dupl,
                timeout,
                dxgi_output,
                dxgi_device,
                dpi_aware,
            }
        };

        Ok(duplicator)
    }

    #[inline]
    pub fn acquire_frame(&mut self) -> Result<(IDXGIResource, DXGI_OUTDUPL_FRAME_INFO)> {
        unsafe {
            let mut frame_info = DXGI_OUTDUPL_FRAME_INFO::default();
            let mut image = None;
            let result =
                self.output_dupl
                    .AcquireNextFrame(self.timeout, &mut frame_info, &mut image);

            if result.is_ok() {
                Ok((image.unwrap(), frame_info))
            } else {
                let error = result.unwrap_err();
                match error.code() {
                    // do nothing and wait until next call if timed out
                    DXGI_ERROR_WAIT_TIMEOUT => (),
                    // must call reset_output_duplicator if AccessLost
                    DXGI_ERROR_ACCESS_LOST => (),
                    // possibly fatal error
                    _ => (),
                }
                Err(error)
            }
        }
    }

    /// Signals that the current frame is done being processed.
    #[inline]
    pub fn release_frame(&mut self) -> Result<()> {
        unsafe {
            self.output_dupl.ReleaseFrame()?;
            Ok(())
        }
    }

    /// Creates a new IDXGIOutputDuplication. Used when access is lost to the display due to
    /// desktop switching or using full-screen programs.
    pub fn reset_output_duplicator(&mut self, supported_formats: &[DXGI_FORMAT]) -> Result<()> {
        let output_dupl = ScreenDuplicator::new_output_duplicator(
            &self.dxgi_output,
            &self.dxgi_device,
            supported_formats,
            self.dpi_aware,
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
        dpi_aware: bool,
    ) -> Result<IDXGIOutputDuplication> {
        // first test if output_dupl can be made with IDXGIOutput5
        if dpi_aware {
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

        // if either not DPI aware or IDXGIOutput5 failed, fall back to IDXGIOutput1
        unsafe {
            let sdr_output: IDXGIOutput1 = dxgi_output.cast()?;
            sdr_output.DuplicateOutput(dxgi_device)
        }
    }

    /// Returns a description of the display being duplicated.
    fn get_display_desc(output_dupl: &IDXGIOutputDuplication) -> DXGI_OUTDUPL_DESC {
        unsafe {
            let mut dupl_desc: MaybeUninit<DXGI_OUTDUPL_DESC> = MaybeUninit::uninit();
            // NOTE: `GetDesc` always succeeds if the `IDXGIOutputDuplication` used is valid
            output_dupl.GetDesc(dupl_desc.as_mut_ptr());
            dupl_desc.assume_init()
        }
    }

    /// Returns the default timeout in milliseconds. It is computed by
    /// taking the ceil of the reciprocal of the refresh rate.
    /// A 60 Hz refresh rate for example will have a default timeout of 17 ms.
    fn compute_default_timeout(output_dupl: &IDXGIOutputDuplication) -> u32 {
        let dupl_desc = ScreenDuplicator::get_display_desc(output_dupl);
        let denom = dupl_desc.ModeDesc.RefreshRate.Denominator;
        let num = dupl_desc.ModeDesc.RefreshRate.Numerator;

        // rounds up the result of the division
        ((1000 * denom - 1) / num) + 1
    }

    /// Attempt to signal to the OS that the process is DPI aware.
    /// Returns the DPI awareness.
    fn try_set_dpi_aware() -> Result<bool> {
        unsafe {
            SetProcessDpiAwareness(PROCESS_PER_MONITOR_DPI_AWARE)?;
            let awareness = GetProcessDpiAwareness(None)?;
            if awareness == PROCESS_SYSTEM_DPI_AWARE || awareness == PROCESS_PER_MONITOR_DPI_AWARE {
                Ok(true)
            } else {
                Ok(false)
            }
        }
    }
}


#[test]
fn print_size() {
    println!("IDXGIResource size: {}", std::mem::size_of::<IDXGIResource>());
}