use core::ptr::slice_from_raw_parts;
use crate::drivers::acpi::acpi_tables::{ACPISignature, AcpiSdtTable};
use crate::drivers::acpi::tables::sdt_header::ACPISDTHeader;

#[repr(C, packed)]
pub struct DSDT {
    header: ACPISDTHeader,
    content: [u8]
}

#[allow(non_snake_case, dead_code)]
pub struct S5Obj {
    pub SLP_TYPa: u32,
    pub SLP_TYPb: u32
}

impl AcpiSdtTable for DSDT {
    fn get_signature(&self) -> ACPISignature {
        ACPISignature::DSDT
    }

    fn validate(&self) -> bool {
        self.get_sdt_header().validate_checksum()
    }

    fn get_sdt_header(&self) -> ACPISDTHeader {
        self.header
    }
}

#[allow(dead_code)]
impl DSDT {
    pub fn new_from_ptr<'a>(ptr: u64) -> &'a DSDT {
        unsafe {
            let header = ACPISDTHeader::new_from_ptr_u64(ptr);
            let length = header.length as usize;
            let rsdt_ptr = slice_from_raw_parts(
                ptr as *const u8,
                length - size_of_val(&header),
            );

            &*(rsdt_ptr as *const DSDT)
        }
    }

    pub fn get_content_length(&self) -> usize {
        let length = self.header.length as usize;
        length - size_of_val(&self.header)
    }

    pub fn get_s5_object_offset(&self) -> Option<usize> {
        //search the AML bytecode for _S5_ string
        let cmp = b"_S5_";
        let content = &self.content;

        for i in 0..=content.len().saturating_sub(cmp.len()) {
            if &content[i..i + cmp.len()] == cmp {
                //S5 string found - now it's time for validation
                //this checks if preceding bytes are valid AML bytecode
                if (content[i-1] == 0x08 || (content[i-2] == 0x08 && content[i-1] == u8::try_from('\\').unwrap()))
                    && content[i+4] == 0x12 {
                    return Some(i);
                }
            }
        }
        None
    }
}

#[allow(non_snake_case, dead_code)]
impl S5Obj {
    pub fn new_from_dsdt(dsdt: &DSDT) -> Option<S5Obj> {
        let mut offset = match dsdt.get_s5_object_offset() {
            Some(x) => x,
            None => return None
        };
        offset += 5; // skip "_S5_" and PackageOp
        offset += ((dsdt.content[offset] as usize & 0xC0) >> 6) + 2; // skip PkgLength


        if dsdt.content[offset] == 0x0A {
            offset += 1;
        }
        let SLP_TYPa = (dsdt.content[offset] as u32) << 10;
        offset += 1;
        if dsdt.content[offset] == 0x0A {
            offset += 1;
        }
        let SLP_TYPb = (dsdt.content[offset] as u32) << 10;

        Some(S5Obj {
            SLP_TYPa,
            SLP_TYPb
        })
    }

}