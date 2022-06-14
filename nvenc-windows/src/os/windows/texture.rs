use windows::Win32::Graphics::{
    Direct3D11::{
        ID3D11Device, ID3D11Texture2D, D3D11_BIND_RENDER_TARGET, D3D11_CPU_ACCESS_FLAG,
        D3D11_RESOURCE_MISC_FLAG, D3D11_TEXTURE2D_DESC, D3D11_USAGE_DEFAULT,
    },
    Dxgi::{Common::DXGI_SAMPLE_DESC, DXGI_OUTDUPL_DESC},
};

/// Creates an `ID3D11Texture2D` where the duplicated frames can be copied to.
pub(crate) fn create_texture_buffer(
    device: &ID3D11Device,
    display_desc: &DXGI_OUTDUPL_DESC,
    buf_size: usize,
) -> windows::core::Result<ID3D11Texture2D> {
    let texture_desc = D3D11_TEXTURE2D_DESC {
        Width: display_desc.ModeDesc.Width,
        Height: display_desc.ModeDesc.Height,
        // plain display output has only one mip
        MipLevels: 1,
        ArraySize: buf_size as u32,
        Format: display_desc.ModeDesc.Format,
        SampleDesc: DXGI_SAMPLE_DESC {
            // default sampler mode
            Count: 1,
            // default sampler mode
            Quality: 0,
        },
        // GPU needs read/write access
        Usage: D3D11_USAGE_DEFAULT,
        // https://github.com/NVIDIA/video-sdk-samples/blob/aa3544dcea2fe63122e4feb83bf805ea40e58dbe/Samples/NvCodec/NvEncoder/NvEncoderD3D11.cpp#L90
        BindFlags: D3D11_BIND_RENDER_TARGET,
        // don't need to be accessed by the CPU
        CPUAccessFlags: D3D11_CPU_ACCESS_FLAG(0),
        MiscFlags: D3D11_RESOURCE_MISC_FLAG(0),
    };

    unsafe {
        device.CreateTexture2D(&texture_desc, std::ptr::null())
    }
}