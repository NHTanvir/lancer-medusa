#![allow(dead_code, unused_imports)]

use alloy_primitives::{Address, I256, U256};
use rand::{Rng, SeedableRng};
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// AbiValue
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AbiValue {
    /// uint8 .. uint256 — (bit_size, value)
    Uint(u8, U256),
    /// int8 .. int256 — (bit_size, value)
    Int(u8, I256),
    Bool(bool),
    Address(Address),
    /// bytes1 .. bytes32 (fixed-size)
    Bytes(Vec<u8>),
    /// bytes (dynamic)
    BytesDynamic(Vec<u8>),
    String(String),
    /// dynamic array T[]
    Array(Vec<AbiValue>),
    /// fixed array T[N]
    FixedArray(Vec<AbiValue>, usize),
    Tuple(Vec<AbiValue>),
}

impl AbiValue {
    /// Encode this value as ABI-encoded bytes suitable for calldata.
    ///
    /// Primitive (static) types are encoded as a single 32-byte word.
    /// Dynamic types (BytesDynamic, String, Array) are encoded as a
    /// simplified 4-byte length prefix followed by raw data — good enough
    /// for the fuzzing phase without implementing the full ABI head/tail
    /// offset scheme.
    pub fn abi_encode(&self) -> Vec<u8> {
        match self {
            // ---- static types --------------------------------------------------
            AbiValue::Uint(_, v) => {
                let mut buf = [0u8; 32];
                let bytes = v.to_be_bytes::<32>();
                buf.copy_from_slice(&bytes);
                buf.to_vec()
            }

            AbiValue::Int(_, v) => {
                // Two's-complement big-endian, sign-extended to 32 bytes.
                let raw: [u8; 32] = v.to_be_bytes();
                raw.to_vec()
            }

            AbiValue::Bool(b) => {
                let mut buf = [0u8; 32];
                buf[31] = if *b { 1 } else { 0 };
                buf.to_vec()
            }

            AbiValue::Address(addr) => {
                // 12 zero bytes + 20 address bytes
                let mut buf = [0u8; 32];
                buf[12..].copy_from_slice(addr.as_slice());
                buf.to_vec()
            }

            AbiValue::Bytes(data) => {
                // Right-padded to 32 bytes (like bytes1..bytes32)
                let mut buf = [0u8; 32];
                let len = data.len().min(32);
                buf[..len].copy_from_slice(&data[..len]);
                buf.to_vec()
            }

            // ---- dynamic types (simplified encoding) ---------------------------
            AbiValue::BytesDynamic(data) => {
                let mut out = Vec::new();
                let len = data.len() as u32;
                out.extend_from_slice(&len.to_be_bytes());
                out.extend_from_slice(data);
                out
            }

            AbiValue::String(s) => {
                let data = s.as_bytes();
                let mut out = Vec::new();
                let len = data.len() as u32;
                out.extend_from_slice(&len.to_be_bytes());
                out.extend_from_slice(data);
                out
            }

            AbiValue::Array(elems) => {
                let mut out = Vec::new();
                let len = elems.len() as u32;
                out.extend_from_slice(&len.to_be_bytes());
                for elem in elems {
                    out.extend_from_slice(&elem.abi_encode());
                }
                out
            }

            AbiValue::FixedArray(elems, _) => {
                let mut out = Vec::new();
                for elem in elems {
                    out.extend_from_slice(&elem.abi_encode());
                }
                out
            }

            AbiValue::Tuple(fields) => {
                let mut out = Vec::new();
                for field in fields {
                    out.extend_from_slice(&field.abi_encode());
                }
                out
            }
        }
    }
}

// ---------------------------------------------------------------------------
// ValueSet
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct ValueSet {
    pub uint_values: Vec<U256>,
    pub int_values: Vec<I256>,
    pub addresses: Vec<Address>,
    pub bytes_values: Vec<Vec<u8>>,
    pub strings: Vec<std::string::String>,
}

impl ValueSet {
    /// Create an empty ValueSet.
    pub fn new() -> Self {
        ValueSet {
            uint_values: Vec::new(),
            int_values: Vec::new(),
            addresses: Vec::new(),
            bytes_values: Vec::new(),
            strings: Vec::new(),
        }
    }

    /// Create a ValueSet pre-seeded with common boundary / interesting values
    /// plus the provided sender addresses.
    pub fn with_defaults(senders: &[Address]) -> Self {
        let mut vs = Self::new();

        // --- uint seeds ---
        vs.uint_values.push(U256::ZERO);
        vs.uint_values.push(U256::from(1u64));
        vs.uint_values.push(U256::from(2u64));
        vs.uint_values.push(U256::MAX);
        // 2^128 - 1
        vs.uint_values.push(
            U256::from(u128::MAX),
        );
        // 2^64 - 1
        vs.uint_values.push(U256::from(u64::MAX));

        // --- int seeds ---
        vs.int_values.push(I256::MIN);
        vs.int_values.push(I256::MAX);
        vs.int_values.push(I256::ZERO);
        if let Ok(v) = I256::try_from(1i64) {
            vs.int_values.push(v);
        }
        if let Ok(v) = I256::try_from(-1i64) {
            vs.int_values.push(v);
        }

        // --- address seeds ---
        vs.addresses.push(Address::ZERO);
        for &addr in senders {
            vs.addresses.push(addr);
        }

        // --- bytes seeds ---
        vs.bytes_values.push(vec![]);
        vs.bytes_values.push(vec![0u8]);
        vs.bytes_values.push(vec![0xFFu8]);

        // --- string seeds ---
        vs.strings.push(std::string::String::new());
        vs.strings.push("a".to_string());
        vs.strings.push("A".repeat(64));

        vs
    }

    pub fn add_uint(&mut self, v: U256) {
        self.uint_values.push(v);
    }

    pub fn add_int(&mut self, v: I256) {
        self.int_values.push(v);
    }

    pub fn add_address(&mut self, a: Address) {
        self.addresses.push(a);
    }

    pub fn add_bytes(&mut self, b: Vec<u8>) {
        self.bytes_values.push(b);
    }

    pub fn add_string(&mut self, s: std::string::String) {
        self.strings.push(s);
    }
}

impl Default for ValueSet {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Helper — mask a U256 to `bit_size` bits
// ---------------------------------------------------------------------------

/// Sentinel: `bit_size == 0` means 256-bit (full width), matching the ABI
/// convention that `uint256` is written as bit_size=0 because 256 overflows u8.
fn effective_bits(bit_size: u8) -> u16 {
    if bit_size == 0 { 256 } else { bit_size as u16 }
}

fn mask_uint(v: U256, bit_size: u8) -> U256 {
    let bits = effective_bits(bit_size);
    if bits >= 256 {
        return v;
    }
    let mask = (U256::from(1u64) << bits as usize) - U256::from(1u64);
    v & mask
}

/// Maximum value for an unsigned integer of `bit_size` bits.
fn uint_max(bit_size: u8) -> U256 {
    let bits = effective_bits(bit_size);
    if bits >= 256 {
        U256::MAX
    } else {
        (U256::from(1u64) << bits as usize) - U256::from(1u64)
    }
}

// ---------------------------------------------------------------------------
// RandomGenerator
// ---------------------------------------------------------------------------

pub struct RandomGenerator {
    rng: rand::rngs::StdRng,
    value_set: ValueSet,
}

impl RandomGenerator {
    pub fn new(seed: u64, value_set: ValueSet) -> Self {
        RandomGenerator {
            rng: rand::rngs::StdRng::seed_from_u64(seed),
            value_set,
        }
    }

    /// Generate a random U256, occasionally drawn from the seed pool.
    pub fn random_uint(&mut self, bit_size: u8) -> U256 {
        // 30% chance: pick from value_set
        if !self.value_set.uint_values.is_empty() && self.rng.gen_bool(0.30) {
            let idx = self.rng.gen_range(0..self.value_set.uint_values.len());
            return mask_uint(self.value_set.uint_values[idx], bit_size);
        }
        // Build random U256 from 4 random u64 limbs, then mask to the right width
        let raw = U256::from_limbs([
            self.rng.gen::<u64>(),
            self.rng.gen::<u64>(),
            self.rng.gen::<u64>(),
            self.rng.gen::<u64>(),
        ]);
        // mask_uint already clamps to [0, 2^bits - 1]; for 256-bit it's a no-op
        mask_uint(raw, bit_size)
    }

    /// Generate a random I256, occasionally drawn from the seed pool.
    pub fn random_int(&mut self, _bit_size: u8) -> I256 {
        if !self.value_set.int_values.is_empty() && self.rng.gen_bool(0.30) {
            let idx = self.rng.gen_range(0..self.value_set.int_values.len());
            return self.value_set.int_values[idx];
        }
        // Generate via random u64 limbs and interpret as signed
        let raw_u256 = U256::from_limbs([
            self.rng.gen::<u64>(),
            self.rng.gen::<u64>(),
            self.rng.gen::<u64>(),
            self.rng.gen::<u64>(),
        ]);
        I256::from_raw(raw_u256)
    }

    /// Return a random address, occasionally from the seed pool.
    pub fn random_address(&mut self) -> Address {
        if !self.value_set.addresses.is_empty() && self.rng.gen_bool(0.40) {
            let idx = self.rng.gen_range(0..self.value_set.addresses.len());
            return self.value_set.addresses[idx];
        }
        let mut bytes = [0u8; 20];
        self.rng.fill(&mut bytes);
        Address::from(bytes)
    }

    /// Random fixed-size bytes of exactly `n` bytes (n <= 32).
    pub fn random_bytes_fixed(&mut self, n: usize) -> Vec<u8> {
        let mut buf = vec![0u8; n.min(32)];
        self.rng.fill(buf.as_mut_slice());
        buf
    }

    /// Random dynamic bytes, occasionally from the seed pool.
    pub fn random_bytes_dynamic(&mut self) -> Vec<u8> {
        if !self.value_set.bytes_values.is_empty() && self.rng.gen_bool(0.20) {
            let idx = self.rng.gen_range(0..self.value_set.bytes_values.len());
            return self.value_set.bytes_values[idx].clone();
        }
        let len = self.rng.gen_range(0usize..=128);
        let mut buf = vec![0u8; len];
        self.rng.fill(buf.as_mut_slice());
        buf
    }

    /// Random string, occasionally from the seed pool.
    pub fn random_string(&mut self) -> std::string::String {
        if !self.value_set.strings.is_empty() && self.rng.gen_bool(0.20) {
            let idx = self.rng.gen_range(0..self.value_set.strings.len());
            return self.value_set.strings[idx].clone();
        }
        let len = self.rng.gen_range(0usize..=64);
        (0..len)
            .map(|_| self.rng.gen_range(b' '..=b'~') as char)
            .collect()
    }

    pub fn random_bool(&mut self) -> bool {
        self.rng.gen_bool(0.5)
    }

    /// Parse `param.ty` and generate a matching `AbiValue`.
    pub fn generate_for_type(&mut self, param: &alloy_json_abi::Param) -> AbiValue {
        generate_for_type_str(self, &param.ty, &param.components)
    }
}

/// Recursive helper that works on the raw type string.
fn generate_for_type_str(
    gen: &mut RandomGenerator,
    ty: &str,
    components: &[alloy_json_abi::Param],
) -> AbiValue {
    // ---- dynamic array: ends with "[]" ------------------------------------
    if let Some(inner_ty) = ty.strip_suffix("[]") {
        let len = gen.rng.gen_range(0usize..=10);
        let elems: Vec<AbiValue> = (0..len)
            .map(|_| generate_for_type_str(gen, inner_ty, components))
            .collect();
        return AbiValue::Array(elems);
    }

    // ---- fixed array: ends with "[N]" -------------------------------------
    if ty.ends_with(']') {
        if let Some(bracket_pos) = ty.rfind('[') {
            let n_str = &ty[bracket_pos + 1..ty.len() - 1];
            if let Ok(n) = n_str.parse::<usize>() {
                let inner_ty = &ty[..bracket_pos];
                let elems: Vec<AbiValue> = (0..n)
                    .map(|_| generate_for_type_str(gen, inner_ty, components))
                    .collect();
                return AbiValue::FixedArray(elems, n);
            }
        }
    }

    // ---- tuple ------------------------------------------------------------
    if ty == "tuple" || ty.starts_with("tuple(") {
        let fields: Vec<AbiValue> = components
            .iter()
            .map(|c| generate_for_type_str(gen, &c.ty, &c.components))
            .collect();
        return AbiValue::Tuple(fields);
    }

    // ---- primitives -------------------------------------------------------
    match ty {
        "bool" => AbiValue::Bool(gen.random_bool()),
        "address" => AbiValue::Address(gen.random_address()),
        "bytes" => AbiValue::BytesDynamic(gen.random_bytes_dynamic()),
        "string" => AbiValue::String(gen.random_string()),

        other => {
            // uint<N>  — bit_size=0 is the sentinel for 256-bit (u8 can't hold 256)
            if let Some(rest) = other.strip_prefix("uint") {
                let bits: u8 = if rest.is_empty() {
                    0 // sentinel for 256
                } else {
                    rest.parse::<u16>().map(|n| if n == 256 { 0u8 } else { n as u8 }).unwrap_or(0)
                };
                return AbiValue::Uint(bits, gen.random_uint(bits));
            }
            // int<N>
            if let Some(rest) = other.strip_prefix("int") {
                let bits: u8 = if rest.is_empty() {
                    0 // sentinel for 256
                } else {
                    rest.parse::<u16>().map(|n| if n == 256 { 0u8 } else { n as u8 }).unwrap_or(0)
                };
                return AbiValue::Int(bits, gen.random_int(bits));
            }
            // bytes<N> (fixed)
            if let Some(rest) = other.strip_prefix("bytes") {
                if let Ok(n) = rest.parse::<usize>() {
                    return AbiValue::Bytes(gen.random_bytes_fixed(n));
                }
            }

            // Unknown / unsupported — safe fallback (0 = 256-bit sentinel)
            AbiValue::Uint(0, U256::ZERO)
        }
    }
}

// ---------------------------------------------------------------------------
// MutationalGenerator
// ---------------------------------------------------------------------------

pub struct MutationalGenerator {
    rng: rand::rngs::StdRng,
    value_set: ValueSet,
}

impl MutationalGenerator {
    pub fn new(seed: u64, value_set: ValueSet) -> Self {
        MutationalGenerator {
            rng: rand::rngs::StdRng::seed_from_u64(seed),
            value_set,
        }
    }

    /// Mutate an `AbiValue` to produce a nearby-but-different value.
    pub fn mutate(&mut self, value: AbiValue) -> AbiValue {
        match value {
            // ---- Uint -------------------------------------------------------
            AbiValue::Uint(bits, v) => {
                let strategy = self.rng.gen_range(0u8..4);
                let result = match strategy {
                    0 => {
                        // add or subtract a small delta
                        let delta = U256::from(self.rng.gen_range(1u64..=256));
                        if self.rng.gen_bool(0.5) {
                            v.saturating_add(delta)
                        } else {
                            v.saturating_sub(delta)
                        }
                    }
                    1 => {
                        // flip a random bit
                        let bit = self.rng.gen_range(0u32..bits as u32);
                        v ^ (U256::from(1u64) << bit as usize)
                    }
                    2 => {
                        // boundary value: 0, 1, or type max
                        match self.rng.gen_range(0u8..3) {
                            0 => U256::ZERO,
                            1 => U256::from(1u64),
                            _ => uint_max(bits),
                        }
                    }
                    _ => {
                        // replace with value from value_set
                        if self.value_set.uint_values.is_empty() {
                            v
                        } else {
                            let idx = self.rng.gen_range(0..self.value_set.uint_values.len());
                            mask_uint(self.value_set.uint_values[idx], bits)
                        }
                    }
                };
                AbiValue::Uint(bits, mask_uint(result, bits))
            }

            // ---- Int --------------------------------------------------------
            AbiValue::Int(bits, v) => {
                let strategy = self.rng.gen_range(0u8..4);
                let result = match strategy {
                    0 => {
                        // add/subtract small delta
                        let delta = I256::try_from(self.rng.gen_range(1i64..=256)).unwrap_or(I256::ZERO);
                        v.saturating_add(delta)
                    }
                    1 => {
                        // flip a random bit in the raw representation
                        let bit = self.rng.gen_range(0u32..bits as u32);
                        let raw = v.into_raw();
                        let flipped = raw ^ (U256::from(1u64) << bit as usize);
                        I256::from_raw(flipped)
                    }
                    2 => {
                        // boundary: MIN, MAX, or ZERO
                        match self.rng.gen_range(0u8..3) {
                            0 => I256::MIN,
                            1 => I256::MAX,
                            _ => I256::ZERO,
                        }
                    }
                    _ => {
                        // replace from value_set
                        if self.value_set.int_values.is_empty() {
                            v
                        } else {
                            let idx = self.rng.gen_range(0..self.value_set.int_values.len());
                            self.value_set.int_values[idx]
                        }
                    }
                };
                AbiValue::Int(bits, result)
            }

            // ---- Bool -------------------------------------------------------
            AbiValue::Bool(b) => AbiValue::Bool(!b),

            // ---- Address ----------------------------------------------------
            AbiValue::Address(_) => {
                if self.value_set.addresses.is_empty() {
                    let mut bytes = [0u8; 20];
                    self.rng.fill(&mut bytes);
                    AbiValue::Address(Address::from(bytes))
                } else {
                    let idx = self.rng.gen_range(0..self.value_set.addresses.len());
                    AbiValue::Address(self.value_set.addresses[idx])
                }
            }

            // ---- Bytes (fixed) ----------------------------------------------
            AbiValue::Bytes(mut data) => {
                if data.is_empty() {
                    return AbiValue::Bytes(data);
                }
                mutate_byte_vec(&mut self.rng, &mut data, false);
                AbiValue::Bytes(data)
            }

            // ---- BytesDynamic -----------------------------------------------
            AbiValue::BytesDynamic(mut data) => {
                mutate_byte_vec(&mut self.rng, &mut data, true);
                AbiValue::BytesDynamic(data)
            }

            // ---- String -----------------------------------------------------
            AbiValue::String(mut s) => {
                let strategy = self.rng.gen_range(0u8..3);
                match strategy {
                    0 => {
                        // shrink: drop last char
                        s.pop();
                    }
                    1 => {
                        // grow: append random printable char
                        let ch = self.rng.gen_range(b' '..=b'~') as char;
                        s.push(ch);
                    }
                    _ => {
                        // mutate one byte (if non-empty and ASCII-safe)
                        if !s.is_empty() {
                            let idx = self.rng.gen_range(0..s.len());
                            let new_byte = self.rng.gen_range(b' '..=b'~');
                            // Safety: we only replace with printable ASCII so UTF-8 stays valid
                            // SAFETY: byte is in 0x20..=0x7E, guaranteed single-byte UTF-8.
                            unsafe {
                                s.as_bytes_mut()[idx] = new_byte;
                            }
                        }
                    }
                }
                AbiValue::String(s)
            }

            // ---- Array (dynamic) --------------------------------------------
            AbiValue::Array(mut elems) => {
                if elems.is_empty() {
                    return AbiValue::Array(elems);
                }
                let strategy = self.rng.gen_range(0u8..4);
                match strategy {
                    0 => {
                        // permute: swap two random elements
                        if elems.len() >= 2 {
                            let i = self.rng.gen_range(0..elems.len());
                            let j = self.rng.gen_range(0..elems.len());
                            elems.swap(i, j);
                        }
                    }
                    1 => {
                        // drop one element
                        let idx = self.rng.gen_range(0..elems.len());
                        elems.remove(idx);
                    }
                    2 => {
                        // duplicate one element
                        let idx = self.rng.gen_range(0..elems.len());
                        let dup = elems[idx].clone();
                        elems.push(dup);
                    }
                    _ => {
                        // mutate one element recursively
                        let idx = self.rng.gen_range(0..elems.len());
                        let elem = elems.remove(idx);
                        let mutated = self.mutate(elem);
                        elems.insert(idx, mutated);
                    }
                }
                AbiValue::Array(elems)
            }

            // ---- FixedArray -------------------------------------------------
            AbiValue::FixedArray(mut elems, n) => {
                if !elems.is_empty() {
                    let idx = self.rng.gen_range(0..elems.len());
                    let elem = elems.remove(idx);
                    let mutated = self.mutate(elem);
                    elems.insert(idx, mutated);
                }
                AbiValue::FixedArray(elems, n)
            }

            // ---- Tuple ------------------------------------------------------
            AbiValue::Tuple(mut fields) => {
                if !fields.is_empty() {
                    let idx = self.rng.gen_range(0..fields.len());
                    let field = fields.remove(idx);
                    let mutated = self.mutate(field);
                    fields.insert(idx, mutated);
                }
                AbiValue::Tuple(fields)
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Internal helper — byte-level mutations shared between Bytes and BytesDynamic
// ---------------------------------------------------------------------------

fn mutate_byte_vec(rng: &mut rand::rngs::StdRng, data: &mut Vec<u8>, allow_resize: bool) {
    if data.is_empty() && !allow_resize {
        return;
    }

    let strategies: u8 = if allow_resize && !data.is_empty() { 4 } else if allow_resize { 2 } else { 2 };
    let strategy = rng.gen_range(0u8..strategies);

    match strategy {
        0 if !data.is_empty() => {
            // arithmetic on one byte: ±1
            let idx = rng.gen_range(0..data.len());
            if rng.gen_bool(0.5) {
                data[idx] = data[idx].wrapping_add(1);
            } else {
                data[idx] = data[idx].wrapping_sub(1);
            }
        }
        1 if !data.is_empty() => {
            // bit flip on one byte
            let idx = rng.gen_range(0..data.len());
            let bit = rng.gen_range(0u8..8);
            data[idx] ^= 1 << bit;
        }
        2 if allow_resize => {
            // grow by 1
            data.push(rng.gen::<u8>());
        }
        3 if allow_resize && !data.is_empty() => {
            // shrink by 1
            data.pop();
        }
        _ => {
            // replace one byte with 0x00 or 0xFF
            if !data.is_empty() {
                let idx = rng.gen_range(0..data.len());
                data[idx] = if rng.gen_bool(0.5) { 0x00 } else { 0xFF };
            }
        }
    }
}
