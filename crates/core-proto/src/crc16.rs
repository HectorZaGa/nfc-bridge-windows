//! Utilidad de cálculo y validación de CRC-16 CCITT (Polinomio 0x1021, Init 0xFFFF).

const POLYNOMIAL: u16 = 0x1021;
const INITIAL_VALUE: u16 = 0xFFFF;

const CRC_TABLE: [u16; 256] = {
    let mut table = [0u16; 256];
    let mut i = 0;
    while i < 256 {
        let mut temp = (i as u16) << 8;
        let mut j = 0;
        while j < 8 {
            if (temp & 0x8000) != 0 {
                temp = (temp << 1) ^ POLYNOMIAL;
            } else {
                temp <<= 1;
            }
            j += 1;
        }
        table[i] = temp;
        i += 1;
    }
    table
};

/// Calcula el CRC-16 para un slice de bytes.
pub fn compute(bytes: &[u8]) -> u16 {
    let mut crc = INITIAL_VALUE;
    for &b in bytes {
        let table_index = (((crc >> 8) ^ (b as u16)) & 0xFF) as usize;
        crc = CRC_TABLE[table_index] ^ ((crc << 8) & 0xFFFF);
    }
    crc
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_crc_empty() {
        assert_eq!(compute(&[]), 0xFFFF);
    }

    #[test]
    fn test_crc_known_vector() {
        let data = b"123456789";
        assert_eq!(compute(data), 0x29B1);
    }
}
