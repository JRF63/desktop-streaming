use windows::{
    core::{Interface, Result},
    Win32::{
        Foundation::BOOL,
        Graphics::{
            Direct3D11::{
                ID3D11Device, ID3D11Device1, ID3D11DeviceContext, ID3D11Query, ID3D11Resource,
                ID3D11Texture2D, D3D11_BIND_FLAG, D3D11_CPU_ACCESS_FLAG, D3D11_QUERY_DESC,
                D3D11_QUERY_EVENT, D3D11_RESOURCE_MISC_SHARED, D3D11_RESOURCE_MISC_SHARED_NTHANDLE,
                D3D11_TEXTURE2D_DESC, D3D11_USAGE_DEFAULT,
            },
            Dxgi::{
                Common::DXGI_SAMPLE_DESC, IDXGIResource, IDXGIResource1, DXGI_OUTDUPL_DESC,
                DXGI_SHARED_RESOURCE_READ,
            },
        },
    },
};
use crossbeam_channel::{Sender, Receiver, bounded};

/// A buffer for the duplicated image.
///
/// This holds and uses an immediate mode context to the passed GPU handle.
/// DirectX API does not allow the same immediate context to be used in two
/// different threads. To use the buffer in another thread, a deferred context
/// the same GPU handle or a different GPU handle to the same device must
/// be used.
pub struct TextureBuffer {
    d3d11_device: ID3D11Device,
    device_context: ID3D11DeviceContext,
    unprocessed_textures: Sender<ID3D11Texture2D>,
    processed_textures: Receiver<ID3D11Texture2D>,
}

impl TextureBuffer {
    /// Creates a new `TextureBuffer`.
    pub fn new(
        d3d11_device: ID3D11Device,
        display_desc: &DXGI_OUTDUPL_DESC,
        num_buffers: usize,
    ) -> Result<(TextureBuffer, Receiver<ID3D11Texture2D>, Sender<ID3D11Texture2D>)> {
        let mut device_context = None;
        unsafe {
            d3d11_device.GetImmediateContext(&mut device_context);
        }
        let (unprocessed_textures, encoder_unprocessed) = bounded(num_buffers);
        let (encoder_processed, processed_textures) = bounded(num_buffers);

        for _ in 0..num_buffers {
            let input_buffer = TextureBuffer::create_input_buffer(
                &d3d11_device,
                display_desc
            )?;
            // TODO: Handle `send` error
            encoder_processed.send(input_buffer).unwrap();
        }

        let texture_buffer = TextureBuffer {
            d3d11_device,
            device_context: device_context.unwrap(),
            unprocessed_textures,
            processed_textures,
        };
        Ok((texture_buffer, encoder_unprocessed, encoder_processed))
    }

    /// Creates an `ID3D11Texture2D` where the duplicated frame can be copied to.
    fn create_input_buffer(
        d3d11_device: &ID3D11Device,
        display_desc: &DXGI_OUTDUPL_DESC,
    ) -> Result<ID3D11Texture2D> {
        let texture_desc = D3D11_TEXTURE2D_DESC {
            Width: display_desc.ModeDesc.Width,
            Height: display_desc.ModeDesc.Height,
            // plain display output has only one mip
            MipLevels: 1,
            ArraySize: 1,
            Format: display_desc.ModeDesc.Format,
            SampleDesc: DXGI_SAMPLE_DESC {
                // default sampler mode
                Count: 1,
                // default sampler mode
                Quality: 0,
            },
            // GPU needs read/write access
            Usage: D3D11_USAGE_DEFAULT,
            // TODO: what flag to use?
            BindFlags: D3D11_BIND_FLAG(0),
            // don't need to be accessed by the CPU
            CPUAccessFlags: D3D11_CPU_ACCESS_FLAG(0),
            // shared with the encoder that has a "different" GPU handle,
            // NTHANDLE to be able to use `CreateSharedHandle` and pass
            // DXGI_SHARED_RESOURCE_READ
            MiscFlags: D3D11_RESOURCE_MISC_SHARED | D3D11_RESOURCE_MISC_SHARED_NTHANDLE,
        };

        unsafe {
            let input_buffer = d3d11_device.CreateTexture2D(&texture_desc, std::ptr::null())?;
            Ok(input_buffer)
        }
    }

    // /// Create a shared handle from this `TextureBuffer`'s device to the other device.
    // pub fn create_shared_handle(
    //     &mut self,
    //     other_device: ComPtr<ID3D11Device>,
    // ) -> Result<ComPtr<ID3D11Texture2D>, WinError> {
    //     let mut resource: ComPtr<IDXGIResource1> = ComPtr::new();
    //     check_result!(self.input_buffer.query_interface(resource.put()));

    //     let mut attributes: SECURITY_ATTRIBUTES = unsafe { std::mem::zeroed() };
    //     attributes.nLength = std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32;

    //     let handle_name: Vec<u16> = vec![0];

    //     unsafe {
    //         check_result!(resource.CreateSharedHandle(
    //             &mut attributes,
    //             DXGI_SHARED_RESOURCE_READ,
    //             handle_name.as_ptr(),
    //             &mut self.shared_handle
    //         ));
    //     }

    //     let mut other_interface: ComPtr<ID3D11Device1> = ComPtr::new();
    //     check_result!(other_device.query_interface(other_interface.put()));

    //     let mut shared_texture: ComPtr<ID3D11Texture2D> = ComPtr::new();
    //     unsafe {
    //         check_result!(other_interface.OpenSharedResource1(
    //             self.shared_handle,
    //             &ID3D11Texture2D::uuidof(),
    //             shared_texture.put() as _
    //         ));
    //     }

    //     Ok(shared_texture)
    // }

    /// Copies the passed resource to the internal texture buffer and returns
    /// its index.
    #[inline]
    pub fn copy_input_frame(&mut self, frame: IDXGIResource) -> Result<()> {
        let acquired_image: ID3D11Texture2D = frame.cast()?;

        // TODO: Handle `recv` error
        let input_buffer = self.processed_textures.recv().unwrap();

        unsafe {
            self.device_context.CopyResource(
                &input_buffer,
                &acquired_image,
            );
        }

        // TODO: Handle `send` error
        self.unprocessed_textures.send(acquired_image).unwrap();

        // self.synchronize_gpu_operation()?;
        Ok(())
    }

    /// GPU operations like CopySubresourceRegion are async and this function
    /// makes it _absolutely_ sure the texture is copied when the GPU accesses
    /// its buffer.
    #[inline(always)]
    fn synchronize_gpu_operation(&mut self) -> Result<()> {
        let mut is_done = BOOL(0);

        let copy_done_desc = D3D11_QUERY_DESC {
            Query: D3D11_QUERY_EVENT,
            MiscFlags: 0,
        };

        let mut flushed = false;

        unsafe {
            let query = self.d3d11_device.CreateQuery(&copy_done_desc)?;
            self.device_context.End(&query);

            loop {
                let query_result = self.device_context.GetData(
                    &query,
                    (&mut is_done as *mut BOOL).cast(),
                    std::mem::size_of::<BOOL>() as u32,
                    0,
                );

                if query_result.is_ok() && is_done.as_bool() {
                    break;
                }

                if !flushed {
                    self.device_context.Flush();
                    flushed = true;
                }
            }
        }

        Ok(())
    }
}
