
use super::*;

#[repr(C)]
pub struct BStr( *mut u16 );

#[cfg(windows)]
#[link(name = "oleaut32")]
extern "system" {
    pub fn SysAllocStringLen(
        psz: *const u16,
        len: u32
    ) -> BStr;
}

#[cfg(not(windows))]
#[allow(non_snake_case)]
pub unsafe fn SysAllocStringLen(
    _psz: *const u16,
    _len: u32
) -> BStr
{
    panic!( "Not implemented" );
}

impl BStr {

    pub fn len_bytes( &self ) -> u32
    {
        unsafe {
            *(( self.0 as usize - 4 ) as *const u32 )
        }
    }

    pub fn string_to_bstr( s : &str ) -> BStr {

        let len = s.len() as u32;
        unsafe {
            SysAllocStringLen(
                s.encode_utf16().collect::<Vec<_>>().as_ptr(),
                len )
        }
    }

    pub fn bstr_to_string( &self ) -> String {

        let slice = unsafe { std::slice::from_raw_parts( 
                self.0 as *const u16,
                ( self.len_bytes() as usize ) / 2 ) };
        String::from_utf16_lossy( slice )
    }
}

impl Default for BStr {
    fn default() -> Self { BStr( std::ptr::null_mut() ) }
}

impl From<BStr> for String {
    fn from( source : BStr ) -> String {
        source.bstr_to_string()
    }
}

impl From<String> for BStr {
    fn from( source : String ) -> BStr {
        BStr::string_to_bstr( &source )
    }
}