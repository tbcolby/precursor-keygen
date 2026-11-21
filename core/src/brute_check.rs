//! Exhaustive check of the password-entropy method on a tiny alphabet.
//! Counting every string directly must agree with inclusion-exclusion.

#[cfg(test)]
mod tests {
    #[test]
    fn inclusion_exclusion_matches_enumeration() {
        // Classes of sizes 2, 1, 1 (alphabet of 4); length 4.
        let class_of = [0usize, 0, 1, 2];
        let sizes = [2usize, 1, 1];
        let len = 4u32;
        let mut direct = 0u32;
        for n in 0..4u32.pow(len) {
            let mut seen = [false; 3];
            let mut x = n;
            for _ in 0..len {
                seen[class_of[(x % 4) as usize]] = true;
                x /= 4;
            }
            if seen.iter().all(|&s| s) {
                direct += 1;
            }
        }
        let mut ie = 0i64;
        for mask in 0u32..8 {
            let ex: usize = (0..3).filter(|i| mask & (1 << i) != 0).map(|i| sizes[i]).sum();
            let t = ((4 - ex) as i64).pow(len);
            ie += if mask.count_ones() % 2 == 0 { t } else { -t };
        }
        assert_eq!(direct as i64, ie);
    }
}
