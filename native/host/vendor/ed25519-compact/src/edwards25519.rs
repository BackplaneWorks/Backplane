use core::cmp::min;
use core::ops::{Add, Sub};

use super::error::*;
use super::field25519::*;

#[derive(Clone, Copy)]
pub struct GeP2 {
    x: Fe,
    y: Fe,
    z: Fe,
}

#[derive(Clone, Copy)]
pub struct GeP3 {
    x: Fe,
    y: Fe,
    z: Fe,
    t: Fe,
}

#[derive(Clone, Copy, Default)]
pub struct GeP1P1 {
    x: Fe,
    y: Fe,
    z: Fe,
    t: Fe,
}

#[derive(Clone, Copy)]
pub struct GePrecomp {
    y_plus_x: Fe,
    y_minus_x: Fe,
    xy2d: Fe,
}

#[derive(Clone, Copy, Default, Eq, PartialEq)]
pub struct GeCached {
    y_plus_x: Fe,
    y_minus_x: Fe,
    z: Fe,
    t2d: Fe,
}

impl GeCached {
    #[cfg_attr(feature = "opt_size", inline(never))]
    #[cfg_attr(not(feature = "opt_size"), inline(always))]
    pub fn maybe_set(&mut self, other: &GeCached, do_swap: u8) {
        self.y_plus_x.maybe_set(&other.y_plus_x, do_swap);
        self.y_minus_x.maybe_set(&other.y_minus_x, do_swap);
        self.z.maybe_set(&other.z, do_swap);
        self.t2d.maybe_set(&other.t2d, do_swap);
    }
}

impl GeP1P1 {
    #[cfg_attr(feature = "opt_size", inline(never))]
    #[cfg_attr(not(feature = "opt_size"), inline(always))]
    fn to_p2(&self) -> GeP2 {
        GeP2 {
            x: self.x * self.t,
            y: self.y * self.z,
            z: self.z * self.t,
        }
    }

    #[cfg_attr(feature = "opt_size", inline(never))]
    #[cfg_attr(not(feature = "opt_size"), inline(always))]
    fn to_p3(&self) -> GeP3 {
        GeP3 {
            x: self.x * self.t,
            y: self.y * self.z,
            z: self.z * self.t,
            t: self.x * self.y,
        }
    }
}

impl GeP2 {
    #[cfg_attr(feature = "opt_size", inline(never))]
    #[cfg_attr(not(feature = "opt_size"), inline(always))]
    fn zero() -> GeP2 {
        GeP2 {
            x: FE_ZERO,
            y: FE_ONE,
            z: FE_ONE,
        }
    }

    #[cfg_attr(feature = "opt_size", inline(never))]
    #[cfg_attr(not(feature = "opt_size"), inline(always))]
    fn dbl(&self) -> GeP1P1 {
        let xx = self.x.square();
        let yy = self.y.square();
        let b = self.z.square_and_double();
        let a = self.x + self.y;
        let aa = a.square();
        let y3 = yy + xx;
        let z3 = yy - xx;
        let x3 = aa - y3;
        let t3 = b - z3;

        GeP1P1 {
            x: x3,
            y: y3,
            z: z3,
            t: t3,
        }
    }

    fn slide(a: &[u8]) -> [i8; 256] {
        let mut r = [0i8; 256];
        for i in 0..256 {
            r[i] = (1 & (a[i >> 3] >> (i & 7))) as i8;
        }
        for i in 0..256 {
            if r[i] != 0 {
                for b in 1..min(7, 256 - i) {
                    if r[i + b] != 0 {
                        if r[i] + (r[i + b] << b) <= 15 {
                            r[i] += r[i + b] << b;
                            r[i + b] = 0;
                        } else if r[i] - (r[i + b] << b) >= -15 {
                            r[i] -= r[i + b] << b;
                            for k in i + b..256 {
                                if r[k] == 0 {
                                    r[k] = 1;
                                    break;
                                }
                                r[k] = 0;
                            }
                        } else {
                            break;
                        }
                    }
                }
            }
        }

        r
    }

    #[allow(clippy::comparison_chain)]
    pub fn double_scalarmult_vartime(a_scalar: &[u8], a_point: GeP3, b_scalar: &[u8]) -> GeP3 {
        let aslide = GeP2::slide(a_scalar);
        let bslide = GeP2::slide(b_scalar);

        let mut ai = [GeCached {
            y_plus_x: FE_ZERO,
            y_minus_x: FE_ZERO,
            z: FE_ZERO,
            t2d: FE_ZERO,
        }; 8]; // A,3A,5A,7A,9A,11A,13A,15A
        ai[0] = a_point.to_cached();
        let a2 = a_point.dbl().to_p3();
        ai[1] = (a2 + ai[0]).to_p3().to_cached();
        ai[2] = (a2 + ai[1]).to_p3().to_cached();
        ai[3] = (a2 + ai[2]).to_p3().to_cached();
        ai[4] = (a2 + ai[3]).to_p3().to_cached();
        ai[5] = (a2 + ai[4]).to_p3().to_cached();
        ai[6] = (a2 + ai[5]).to_p3().to_cached();
        ai[7] = (a2 + ai[6]).to_p3().to_cached();

        let mut r = GeP2::zero();

        let mut i: usize = 255;
        loop {
            if aslide[i] != 0 || bslide[i] != 0 {
                break;
            }
            if i == 0 {
                return GeP3::zero();
            }
            i -= 1;
        }

        loop {
            let mut t = r.dbl();
            if aslide[i] > 0 {
                t = t.to_p3() + ai[(aslide[i] / 2) as usize];
            } else if aslide[i] < 0 {
                t = t.to_p3() - ai[(-aslide[i] / 2) as usize];
            }

            if bslide[i] > 0 {
                t = t.to_p3() + BI[(bslide[i] / 2) as usize];
            } else if bslide[i] < 0 {
                t = t.to_p3() - BI[(-bslide[i] / 2) as usize];
            }

            if i == 0 {
                return t.to_p3();
            }
            r = t.to_p2();
            i -= 1;
        }
    }
}

impl GeP3 {
    pub fn from_bytes_negate_vartime(s: &[u8; 32]) -> Option<GeP3> {
        let mut encoded_y = *s;
        encoded_y[31] &= 0x7f;
        Fe::reject_noncanonical(&encoded_y).ok()?;

        let y = Fe::from_bytes(s);
        let z = FE_ONE;
        let y_squared = y.square();
        let u = y_squared - FE_ONE;
        let v = (y_squared * FE_D) + FE_ONE;
        let mut x = (u * v).pow25523() * u;

        let vxx = x.square() * v;
        let check = vxx - u;
        if !check.is_zero() {
            let check2 = vxx + u;
            if !check2.is_zero() {
                return None;
            }
            x = x * FE_SQRTM1;
        }

        if x.is_negative() == ((s[31] >> 7) != 0) {
            x = x.neg();
        }

        if x.is_zero() && s[31] >> 7 != 0 {
            return None;
        }

        let t = x * y;

        Some(GeP3 { x, y, z, t })
    }

    pub fn from_bytes_vartime(s: &[u8; 32]) -> Option<GeP3> {
        Self::from_bytes_negate_vartime(s).map(|p| GeP3 {
            x: p.x.neg(),
            y: p.y,
            z: p.z,
            t: p.t.neg(),
        })
    }

    #[cfg_attr(feature = "opt_size", inline(never))]
    #[cfg_attr(not(feature = "opt_size"), inline(always))]
    fn to_p2(&self) -> GeP2 {
        GeP2 {
            x: self.x,
            y: self.y,
            z: self.z,
        }
    }

    #[cfg_attr(feature = "opt_size", inline(never))]
    #[cfg_attr(not(feature = "opt_size"), inline(always))]
    fn to_cached(&self) -> GeCached {
        GeCached {
            y_plus_x: self.y + self.x,
            y_minus_x: self.y - self.x,
            z: self.z,
            t2d: self.t * FE_D2,
        }
    }

    #[cfg_attr(feature = "opt_size", inline(never))]
    #[cfg_attr(not(feature = "opt_size"), inline(always))]
    fn zero() -> GeP3 {
        GeP3 {
            x: FE_ZERO,
            y: FE_ONE,
            z: FE_ONE,
            t: FE_ZERO,
        }
    }

    #[cfg_attr(feature = "opt_size", inline(never))]
    #[cfg_attr(not(feature = "opt_size"), inline(always))]
    fn dbl(&self) -> GeP1P1 {
        self.to_p2().dbl()
    }

    pub fn to_bytes(&self) -> [u8; 32] {
        let recip = self.z.invert();
        let x = self.x * recip;
        let y = self.y * recip;
        let mut bs = y.to_bytes();
        bs[31] ^= (if x.is_negative() { 1 } else { 0 }) << 7;
        bs
    }

    // Only small-order points have x = 0 once multiplied by 4.
    pub fn has_small_order(&self) -> bool {
        self.dbl().to_p2().dbl().x.is_zero()
    }
}

impl Add<GeP3> for GeP3 {
    type Output = GeP3;

    #[cfg_attr(feature = "opt_size", inline(never))]
    #[cfg_attr(not(feature = "opt_size"), inline(always))]
    fn add(self, other: GeP3) -> GeP3 {
        (self + other.to_cached()).to_p3()
    }
}

impl Sub<GeP3> for GeP3 {
    type Output = GeP3;

    #[cfg_attr(feature = "opt_size", inline(never))]
    #[cfg_attr(not(feature = "opt_size"), inline(always))]
    fn sub(self, other: GeP3) -> GeP3 {
        (self - other.to_cached()).to_p3()
    }
}

impl Add<GeCached> for GeP3 {
    type Output = GeP1P1;

    #[cfg_attr(feature = "opt_size", inline(never))]
    #[cfg_attr(not(feature = "opt_size"), inline(always))]
    fn add(self, _rhs: GeCached) -> GeP1P1 {
        let y1_plus_x1 = self.y + self.x;
        let y1_minus_x1 = self.y - self.x;
        let a = y1_plus_x1 * _rhs.y_plus_x;
        let b = y1_minus_x1 * _rhs.y_minus_x;
        let c = _rhs.t2d * self.t;
        let zz = self.z * _rhs.z;
        let d = zz + zz;
        let x3 = a - b;
        let y3 = a + b;
        let z3 = d + c;
        let t3 = d - c;

        GeP1P1 {
            x: x3,
            y: y3,
            z: z3,
            t: t3,
        }
    }
}

impl Add<GePrecomp> for GeP3 {
    type Output = GeP1P1;

    #[cfg_attr(feature = "opt_size", inline(never))]
    #[cfg_attr(not(feature = "opt_size"), inline(always))]
    fn add(self, _rhs: GePrecomp) -> GeP1P1 {
        let y1_plus_x1 = self.y + self.x;
        let y1_minus_x1 = self.y - self.x;
        let a = y1_plus_x1 * _rhs.y_plus_x;
        let b = y1_minus_x1 * _rhs.y_minus_x;
        let c = _rhs.xy2d * self.t;
        let d = self.z + self.z;
        let x3 = a - b;
        let y3 = a + b;
        let z3 = d + c;
        let t3 = d - c;

        GeP1P1 {
            x: x3,
            y: y3,
            z: z3,
            t: t3,
        }
    }
}

impl Sub<GeCached> for GeP3 {
    type Output = GeP1P1;

    #[cfg_attr(feature = "opt_size", inline(never))]
    #[cfg_attr(not(feature = "opt_size"), inline(always))]
    fn sub(self, _rhs: GeCached) -> GeP1P1 {
        let y1_plus_x1 = self.y + self.x;
        let y1_minus_x1 = self.y - self.x;
        let a = y1_plus_x1 * _rhs.y_minus_x;
        let b = y1_minus_x1 * _rhs.y_plus_x;
        let c = _rhs.t2d * self.t;
        let zz = self.z * _rhs.z;
        let d = zz + zz;
        let x3 = a - b;
        let y3 = a + b;
        let z3 = d - c;
        let t3 = d + c;

        GeP1P1 {
            x: x3,
            y: y3,
            z: z3,
            t: t3,
        }
    }
}

impl Sub<GePrecomp> for GeP3 {
    type Output = GeP1P1;

    #[cfg_attr(feature = "opt_size", inline(never))]
    #[cfg_attr(not(feature = "opt_size"), inline(always))]
    fn sub(self, _rhs: GePrecomp) -> GeP1P1 {
        let y1_plus_x1 = self.y + self.x;
        let y1_minus_x1 = self.y - self.x;
        let a = y1_plus_x1 * _rhs.y_minus_x;
        let b = y1_minus_x1 * _rhs.y_plus_x;
        let c = _rhs.xy2d * self.t;
        let d = self.z + self.z;
        let x3 = a - b;
        let y3 = a + b;
        let z3 = d - c;
        let t3 = d + c;

        GeP1P1 {
            x: x3,
            y: y3,
            z: z3,
            t: t3,
        }
    }
}

#[cfg(any(feature = "blind-keys", feature = "opt_size", test))]
fn ge_precompute(base: &GeP3) -> [GeCached; 16] {
    let base_cached = base.to_cached();
    let mut pc = [GeP3::zero(); 16];
    pc[1] = *base;
    for i in 2..16 {
        pc[i] = if i % 2 == 0 {
            pc[i / 2].dbl().to_p3()
        } else {
            pc[i - 1].add(base_cached).to_p3()
        }
    }
    let mut pc_cached: [GeCached; 16] = Default::default();
    for i in 0..16 {
        pc_cached[i] = pc[i].to_cached();
    }
    pc_cached
}

#[cfg(any(feature = "blind-keys", feature = "opt_size", test))]
pub fn ge_scalarmult(scalar: &[u8], base: &GeP3) -> GeP3 {
    let pc = ge_precompute(base);
    ge_scalarmult_precomputed(scalar, &pc)
}

fn ge_scalarmult_precomputed(scalar: &[u8], pc: &[GeCached; 16]) -> GeP3 {
    let mut q = GeP3::zero();
    let mut pos = 252;
    loop {
        let slot = ((scalar[pos >> 3] >> (pos & 7)) & 15) as usize;
        let mut t = pc[0];
        for i in 1..16 {
            t.maybe_set(&pc[i], (((slot ^ i).wrapping_sub(1)) >> 8) as u8 & 1);
        }
        q = q.add(t).to_p3();
        if pos == 0 {
            break;
        }
        q = q.dbl().to_p3().dbl().to_p3().dbl().to_p3().dbl().to_p3();
        pos -= 4;
    }
    q
}

#[cfg(not(feature = "opt_size"))]
static BASEPOINT_PC: [GeCached; 16] = [
    GeCached {
        y_plus_x: Fe([1, 0, 0, 0, 0]),
        y_minus_x: Fe([
            2251799813685230,
            2251799813685247,
            2251799813685247,
            2251799813685247,
            2251799813685247,
        ]),
        z: Fe([1, 0, 0, 0, 0]),
        t2d: Fe([0, 0, 0, 0, 0]),
    },
    GeCached {
        y_plus_x: Fe([
            3540182452943730,
            2497478415033846,
            2521227595762870,
            1462984067271729,
            2389212253076811,
        ]),
        y_minus_x: Fe([
            62697248952638,
            204681361388450,
            631292143396476,
            338455783676468,
            1213667448819585,
        ]),
        z: Fe([1, 0, 0, 0, 0]),
        t2d: Fe([
            301289933810280,
            1259582250014073,
            1422107436869536,
            796239922652654,
            1953934009299142,
        ]),
    },
    GeCached {
        y_plus_x: Fe([
            2899251262813612,
            2105814100803166,
            1713949677790798,
            3215891443006982,
            2739384323378089,
        ]),
        y_minus_x: Fe([
            2068674118847085,
            1963549090372877,
            1018819090357341,
            202036157367742,
            2216582574168879,
        ]),
        z: Fe([
            939820407267714,
            2244711721325457,
            833935350215291,
            514982476710626,
            395358860036744,
        ]),
        t2d: Fe([
            480126441932807,
            360939894744223,
            414963024589262,
            1497007729610176,
            631704639649473,
        ]),
    },
    GeCached {
        y_plus_x: Fe([
            3203379641193801,
            2671375269558872,
            2070381889117393,
            3408704250464255,
            2000447963965959,
        ]),
        y_minus_x: Fe([
            904464151500333,
            1771035278322932,
            211167236544955,
            1536100403113214,
            1726723339398472,
        ]),
        z: Fe([
            1061705710463460,
            150884959185907,
            158377546300656,
            2007006659873485,
            1408482789065644,
        ]),
        t2d: Fe([
            1742083302726619,
            1616172736080930,
            1275485487654045,
            37675288128703,
            2095576491262187,
        ]),
    },
    GeCached {
        y_plus_x: Fe([
            1695410796818793,
            2731695111583170,
            2714556126557059,
            451274161496800,
            2012078832798171,
        ]),
        y_minus_x: Fe([
            375025409391801,
            169834297521966,
            387980374534797,
            334628214200116,
            1830506172326927,
        ]),
        z: Fe([
            1904279982576693,
            1467233916658614,
            1298643500842388,
            324859978027951,
            133149465829202,
        ]),
        t2d: Fe([
            1995200425888795,
            1351231165224936,
            2176876606696969,
            1436679290588863,
            1709317355911061,
        ]),
    },
    GeCached {
        y_plus_x: Fe([
            2926516234834723,
            1450717128675707,
            3397919271948925,
            979738693950965,
            1347227353637357,
        ]),
        y_minus_x: Fe([
            1410465607609101,
            1665000734678196,
            803676666699956,
            2236342313567835,
            1245809528730440,
        ]),
        z: Fe([
            882236476409592,
            1497675105009012,
            2181927182817440,
            722619068391161,
            819202421408532,
        ]),
        t2d: Fe([
            546543842467106,
            1449778946980981,
            892502568874740,
            2186914323197665,
            1183292400222437,
        ]),
    },
    GeCached {
        y_plus_x: Fe([
            2609877896546266,
            1096697072067135,
            1971135628989201,
            3414701313108708,
            4210899004685168,
        ]),
        y_minus_x: Fe([
            599697439270285,
            1382998135852290,
            771968960200254,
            1878135829456840,
            2103521242054977,
        ]),
        z: Fe([
            406273932530493,
            1601638185061342,
            1151347692721435,
            1495804819323688,
            118365337991367,
        ]),
        t2d: Fe([
            783552521469343,
            550229790668946,
            2052808141901586,
            1760428542905596,
            1928764989073059,
        ]),
    },
    GeCached {
        y_plus_x: Fe([
            2159545607990893,
            3147448510622730,
            486881527044235,
            2248590592936728,
            1895208328891224,
        ]),
        y_minus_x: Fe([
            2186604932016262,
            1649225228573991,
            1873456856841000,
            1099532248606737,
            2213142988759011,
        ]),
        z: Fe([
            960540512985497,
            1426503578465076,
            2188424813616240,
            1795326679408039,
            1607151223114465,
        ]),
        t2d: Fe([
            1747734643317249,
            743380680120617,
            1580101169068086,
            1087664216424501,
            277545570925331,
        ]),
    },
    GeCached {
        y_plus_x: Fe([
            3043749755220433,
            1938598694900172,
            3049626199986415,
            3047949431791564,
            2789682914382751,
        ]),
        y_minus_x: Fe([
            828387156123051,
            243955621460461,
            762099155843483,
            946477271729806,
            1701652729669002,
        ]),
        z: Fe([
            207037345464830,
            1993080251615457,
            560932193579569,
            999626924140364,
            754134581836708,
        ]),
        t2d: Fe([
            482766940000144,
            33540757945105,
            749360811823239,
            1460340387593593,
            1757947279433989,
        ]),
    },
    GeCached {
        y_plus_x: Fe([
            2240304515960624,
            3537386921097767,
            2695895682995147,
            1811157917985109,
            2779938286529648,
        ]),
        y_minus_x: Fe([
            1022781769132038,
            585623358173500,
            709581212592317,
            836667937950586,
            1408009946183497,
        ]),
        z: Fe([
            1934056789485370,
            2187228471026461,
            79683303172571,
            246283662803711,
            749534771196139,
        ]),
        t2d: Fe([
            540262790467214,
            290190952004667,
            216950454494661,
            1630941179060970,
            785199203470672,
        ]),
    },
    GeCached {
        y_plus_x: Fe([
            1512916939093555,
            2992645879250201,
            2528509081340787,
            2131332975592474,
            761336180782299,
        ]),
        y_minus_x: Fe([
            405457505626004,
            1019703549184813,
            1916987915502681,
            1839883741534417,
            2193392785174467,
        ]),
        z: Fe([
            1818564342032891,
            1136491851523721,
            35093607622580,
            1441385653535882,
            732801009348817,
        ]),
        t2d: Fe([
            253284364260683,
            1123031426303723,
            1341521644054804,
            1010135543336479,
            1479524374320333,
        ]),
    },
    GeCached {
        y_plus_x: Fe([
            2365398187489437,
            1124628442973815,
            2499161664361135,
            1471589956131712,
            1510258093099827,
        ]),
        y_minus_x: Fe([
            905705748835765,
            1202252692568785,
            1536609018933038,
            1804479828235729,
            308758094835482,
        ]),
        z: Fe([
            260719394240982,
            249358397423578,
            1898576669281045,
            1319825585617897,
            2203484913493586,
        ]),
        t2d: Fe([
            1074592667060695,
            2190819092840175,
            1113659602279329,
            514588423576764,
            1594850183602973,
        ]),
    },
    GeCached {
        y_plus_x: Fe([
            363425857861601,
            1734418183714081,
            4179723901978219,
            2926060052259184,
            3820257750753629,
        ]),
        y_minus_x: Fe([
            2088069355629081,
            2131073356111154,
            273789259070562,
            1547386807830052,
            3563777439570,
        ]),
        z: Fe([
            207103967810401,
            768727957346634,
            718644737663762,
            1362212140629448,
            1529120863451279,
        ]),
        t2d: Fe([
            989664194820699,
            1717065545893193,
            1514630822111268,
            1510465000922594,
            411575651130149,
        ]),
    },
    GeCached {
        y_plus_x: Fe([
            2431640481383299,
            3535729509476129,
            3035644312764025,
            2313551322106050,
            2935089592225046,
        ]),
        y_minus_x: Fe([
            765935297493716,
            649629965652465,
            1104455927412399,
            95176349810862,
            1242329781295592,
        ]),
        z: Fe([
            2250000172488624,
            1617330080438570,
            1107263496438848,
            2112155113306328,
            980993448222351,
        ]),
        t2d: Fe([
            862537504556532,
            566266187869470,
            2038862002856850,
            803960893275491,
            1297159503099026,
        ]),
    },
    GeCached {
        y_plus_x: Fe([
            2315993817135055,
            2652722761171029,
            2522401301012109,
            1075741408880579,
            1706513857602015,
        ]),
        y_minus_x: Fe([
            311097472864451,
            1284388470688736,
            1100455435689831,
            1913025024780403,
            1582956969378808,
        ]),
        z: Fe([
            1081431594790383,
            1952209085138192,
            1101000417182974,
            1343359737131623,
            837835975770184,
        ]),
        t2d: Fe([
            1468367993771685,
            1651387822379794,
            2229056712608003,
            1415674651251816,
            903055878175404,
        ]),
    },
    GeCached {
        y_plus_x: Fe([
            2597095379176553,
            3281739572249654,
            3002841564715431,
            204337338762971,
            578630945781619,
        ]),
        y_minus_x: Fe([
            168655884267790,
            1669918081707416,
            420990948374884,
            2141888286050751,
            2040850418048232,
        ]),
        z: Fe([
            684290857876024,
            657789607835756,
            1569561823708010,
            1217543024068483,
            2219703400977787,
        ]),
        t2d: Fe([
            900369721197708,
            875098376162073,
            666599403867478,
            1595664697968551,
            990948962180400,
        ]),
    },
];

pub fn ge_scalarmult_base(scalar: &[u8]) -> GeP3 {
    #[cfg(not(feature = "opt_size"))]
    {
        return ge_scalarmult_precomputed(scalar, &BASEPOINT_PC);
    }

    #[cfg(feature = "opt_size")]
    {
        const BXP: [u8; 32] = [
            0x1a, 0xd5, 0x25, 0x8f, 0x60, 0x2d, 0x56, 0xc9, 0xb2, 0xa7, 0x25, 0x95, 0x60, 0xc7,
            0x2c, 0x69, 0x5c, 0xdc, 0xd6, 0xfd, 0x31, 0xe2, 0xa4, 0xc0, 0xfe, 0x53, 0x6e, 0xcd,
            0xd3, 0x36, 0x69, 0x21,
        ];
        const BYP: [u8; 32] = [
            0x58, 0x66, 0x66, 0x66, 0x66, 0x66, 0x66, 0x66, 0x66, 0x66, 0x66, 0x66, 0x66, 0x66,
            0x66, 0x66, 0x66, 0x66, 0x66, 0x66, 0x66, 0x66, 0x66, 0x66, 0x66, 0x66, 0x66, 0x66,
            0x66, 0x66, 0x66, 0x66,
        ];
        let bx = Fe::from_bytes(&BXP);
        let by = Fe::from_bytes(&BYP);
        let base = GeP3 {
            x: bx,
            y: by,
            z: FE_ONE,
            t: bx * by,
        };
        ge_scalarmult(scalar, &base)
    }
}

#[cfg(all(test, not(feature = "opt_size")))]
#[test]
fn test_basepoint_pc_matches_runtime_precompute() {
    const BXP: [u8; 32] = [
        0x1a, 0xd5, 0x25, 0x8f, 0x60, 0x2d, 0x56, 0xc9, 0xb2, 0xa7, 0x25, 0x95, 0x60, 0xc7, 0x2c,
        0x69, 0x5c, 0xdc, 0xd6, 0xfd, 0x31, 0xe2, 0xa4, 0xc0, 0xfe, 0x53, 0x6e, 0xcd, 0xd3, 0x36,
        0x69, 0x21,
    ];
    const BYP: [u8; 32] = [
        0x58, 0x66, 0x66, 0x66, 0x66, 0x66, 0x66, 0x66, 0x66, 0x66, 0x66, 0x66, 0x66, 0x66, 0x66,
        0x66, 0x66, 0x66, 0x66, 0x66, 0x66, 0x66, 0x66, 0x66, 0x66, 0x66, 0x66, 0x66, 0x66, 0x66,
        0x66, 0x66,
    ];
    let bx = Fe::from_bytes(&BXP);
    let by = Fe::from_bytes(&BYP);
    let base = GeP3 {
        x: bx,
        y: by,
        z: FE_ONE,
        t: bx * by,
    };
    assert!(BASEPOINT_PC == ge_precompute(&base));
    for scalar in [
        [0u8; 32],
        [1u8; 32],
        [42u8; 32],
        [0xffu8; 32],
        [
            0x70, 0x34, 0x1d, 0x7d, 0xa2, 0x31, 0x50, 0xc3, 0x0f, 0xb8, 0x76, 0x45, 0x29, 0xc2,
            0x17, 0xa3, 0x85, 0x1a, 0x61, 0x50, 0xb7, 0xa1, 0xda, 0x79, 0x08, 0xbe, 0x3a, 0xd6,
            0xcc, 0x94, 0xf4, 0x03,
        ],
    ] {
        assert_eq!(
            ge_scalarmult_precomputed(&scalar, &BASEPOINT_PC).to_bytes(),
            ge_scalarmult(&scalar, &base).to_bytes()
        );
    }
}

#[cfg(feature = "x25519")]
pub fn ge_to_x25519_vartime(s: &[u8; 32]) -> Option<[u8; 32]> {
    let p = GeP3::from_bytes_vartime(s)?;
    let yed = p.y;
    let x_mont = (FE_ONE + yed) * ((FE_ONE - yed).invert());
    Some(x_mont.to_bytes())
}

pub fn sc_reduce32(s: &mut [u8; 32]) {
    let mut t = [0u8; 64];
    t[0..32].copy_from_slice(s);
    sc_reduce(&mut t);
    s.copy_from_slice(&t[0..32]);
}

pub fn sc_reduce(s: &mut [u8]) {
    let mut s0: i64 = 2097151 & load_3i(s);
    let mut s1: i64 = 2097151 & (load_4i(&s[2..6]) >> 5);
    let mut s2: i64 = 2097151 & (load_3i(&s[5..8]) >> 2);
    let mut s3: i64 = 2097151 & (load_4i(&s[7..11]) >> 7);
    let mut s4: i64 = 2097151 & (load_4i(&s[10..14]) >> 4);
    let mut s5: i64 = 2097151 & (load_3i(&s[13..16]) >> 1);
    let mut s6: i64 = 2097151 & (load_4i(&s[15..19]) >> 6);
    let mut s7: i64 = 2097151 & (load_3i(&s[18..21]) >> 3);
    let mut s8: i64 = 2097151 & load_3i(&s[21..24]);
    let mut s9: i64 = 2097151 & (load_4i(&s[23..27]) >> 5);
    let mut s10: i64 = 2097151 & (load_3i(&s[26..29]) >> 2);
    let mut s11: i64 = 2097151 & (load_4i(&s[28..32]) >> 7);
    let mut s12: i64 = 2097151 & (load_4i(&s[31..35]) >> 4);
    let mut s13: i64 = 2097151 & (load_3i(&s[34..37]) >> 1);
    let mut s14: i64 = 2097151 & (load_4i(&s[36..40]) >> 6);
    let mut s15: i64 = 2097151 & (load_3i(&s[39..42]) >> 3);
    let mut s16: i64 = 2097151 & load_3i(&s[42..45]);
    let mut s17: i64 = 2097151 & (load_4i(&s[44..48]) >> 5);
    let s18: i64 = 2097151 & (load_3i(&s[47..50]) >> 2);
    let s19: i64 = 2097151 & (load_4i(&s[49..53]) >> 7);
    let s20: i64 = 2097151 & (load_4i(&s[52..56]) >> 4);
    let s21: i64 = 2097151 & (load_3i(&s[55..58]) >> 1);
    let s22: i64 = 2097151 & (load_4i(&s[57..61]) >> 6);
    let s23: i64 = load_4i(&s[60..64]) >> 3;

    s11 += s23 * 666643;
    s12 += s23 * 470296;
    s13 += s23 * 654183;
    s14 -= s23 * 997805;
    s15 += s23 * 136657;
    s16 -= s23 * 683901;

    s10 += s22 * 666643;
    s11 += s22 * 470296;
    s12 += s22 * 654183;
    s13 -= s22 * 997805;
    s14 += s22 * 136657;
    s15 -= s22 * 683901;

    s9 += s21 * 666643;
    s10 += s21 * 470296;
    s11 += s21 * 654183;
    s12 -= s21 * 997805;
    s13 += s21 * 136657;
    s14 -= s21 * 683901;

    s8 += s20 * 666643;
    s9 += s20 * 470296;
    s10 += s20 * 654183;
    s11 -= s20 * 997805;
    s12 += s20 * 136657;
    s13 -= s20 * 683901;

    s7 += s19 * 666643;
    s8 += s19 * 470296;
    s9 += s19 * 654183;
    s10 -= s19 * 997805;
    s11 += s19 * 136657;
    s12 -= s19 * 683901;

    s6 += s18 * 666643;
    s7 += s18 * 470296;
    s8 += s18 * 654183;
    s9 -= s18 * 997805;
    s10 += s18 * 136657;
    s11 -= s18 * 683901;

    let mut carry6: i64 = (s6 + (1 << 20)) >> 21;
    s7 += carry6;
    s6 -= carry6 << 21;
    let mut carry8: i64 = (s8 + (1 << 20)) >> 21;
    s9 += carry8;
    s8 -= carry8 << 21;
    let mut carry10: i64 = (s10 + (1 << 20)) >> 21;
    s11 += carry10;
    s10 -= carry10 << 21;
    let carry12: i64 = (s12 + (1 << 20)) >> 21;
    s13 += carry12;
    s12 -= carry12 << 21;
    let carry14: i64 = (s14 + (1 << 20)) >> 21;
    s15 += carry14;
    s14 -= carry14 << 21;
    let carry16: i64 = (s16 + (1 << 20)) >> 21;
    s17 += carry16;
    s16 -= carry16 << 21;

    let mut carry7: i64 = (s7 + (1 << 20)) >> 21;
    s8 += carry7;
    s7 -= carry7 << 21;
    let mut carry9: i64 = (s9 + (1 << 20)) >> 21;
    s10 += carry9;
    s9 -= carry9 << 21;
    let mut carry11: i64 = (s11 + (1 << 20)) >> 21;
    s12 += carry11;
    s11 -= carry11 << 21;
    let carry13: i64 = (s13 + (1 << 20)) >> 21;
    s14 += carry13;
    s13 -= carry13 << 21;
    let carry15: i64 = (s15 + (1 << 20)) >> 21;
    s16 += carry15;
    s15 -= carry15 << 21;

    s5 += s17 * 666643;
    s6 += s17 * 470296;
    s7 += s17 * 654183;
    s8 -= s17 * 997805;
    s9 += s17 * 136657;
    s10 -= s17 * 683901;

    s4 += s16 * 666643;
    s5 += s16 * 470296;
    s6 += s16 * 654183;
    s7 -= s16 * 997805;
    s8 += s16 * 136657;
    s9 -= s16 * 683901;

    s3 += s15 * 666643;
    s4 += s15 * 470296;
    s5 += s15 * 654183;
    s6 -= s15 * 997805;
    s7 += s15 * 136657;
    s8 -= s15 * 683901;

    s2 += s14 * 666643;
    s3 += s14 * 470296;
    s4 += s14 * 654183;
    s5 -= s14 * 997805;
    s6 += s14 * 136657;
    s7 -= s14 * 683901;

    s1 += s13 * 666643;
    s2 += s13 * 470296;
    s3 += s13 * 654183;
    s4 -= s13 * 997805;
    s5 += s13 * 136657;
    s6 -= s13 * 683901;

    s0 += s12 * 666643;
    s1 += s12 * 470296;
    s2 += s12 * 654183;
    s3 -= s12 * 997805;
    s4 += s12 * 136657;
    s5 -= s12 * 683901;
    s12 = 0;

    let mut carry0: i64 = (s0 + (1 << 20)) >> 21;
    s1 += carry0;
    s0 -= carry0 << 21;
    let mut carry2: i64 = (s2 + (1 << 20)) >> 21;
    s3 += carry2;
    s2 -= carry2 << 21;
    let mut carry4: i64 = (s4 + (1 << 20)) >> 21;
    s5 += carry4;
    s4 -= carry4 << 21;
    carry6 = (s6 + (1 << 20)) >> 21;
    s7 += carry6;
    s6 -= carry6 << 21;
    carry8 = (s8 + (1 << 20)) >> 21;
    s9 += carry8;
    s8 -= carry8 << 21;
    carry10 = (s10 + (1 << 20)) >> 21;
    s11 += carry10;
    s10 -= carry10 << 21;

    let mut carry1: i64 = (s1 + (1 << 20)) >> 21;
    s2 += carry1;
    s1 -= carry1 << 21;
    let mut carry3: i64 = (s3 + (1 << 20)) >> 21;
    s4 += carry3;
    s3 -= carry3 << 21;
    let mut carry5: i64 = (s5 + (1 << 20)) >> 21;
    s6 += carry5;
    s5 -= carry5 << 21;
    carry7 = (s7 + (1 << 20)) >> 21;
    s8 += carry7;
    s7 -= carry7 << 21;
    carry9 = (s9 + (1 << 20)) >> 21;
    s10 += carry9;
    s9 -= carry9 << 21;
    carry11 = (s11 + (1 << 20)) >> 21;
    s12 += carry11;
    s11 -= carry11 << 21;

    s0 += s12 * 666643;
    s1 += s12 * 470296;
    s2 += s12 * 654183;
    s3 -= s12 * 997805;
    s4 += s12 * 136657;
    s5 -= s12 * 683901;
    s12 = 0;

    carry0 = s0 >> 21;
    s1 += carry0;
    s0 -= carry0 << 21;
    carry1 = s1 >> 21;
    s2 += carry1;
    s1 -= carry1 << 21;
    carry2 = s2 >> 21;
    s3 += carry2;
    s2 -= carry2 << 21;
    carry3 = s3 >> 21;
    s4 += carry3;
    s3 -= carry3 << 21;
    carry4 = s4 >> 21;
    s5 += carry4;
    s4 -= carry4 << 21;
    carry5 = s5 >> 21;
    s6 += carry5;
    s5 -= carry5 << 21;
    carry6 = s6 >> 21;
    s7 += carry6;
    s6 -= carry6 << 21;
    carry7 = s7 >> 21;
    s8 += carry7;
    s7 -= carry7 << 21;
    carry8 = s8 >> 21;
    s9 += carry8;
    s8 -= carry8 << 21;
    carry9 = s9 >> 21;
    s10 += carry9;
    s9 -= carry9 << 21;
    carry10 = s10 >> 21;
    s11 += carry10;
    s10 -= carry10 << 21;
    carry11 = s11 >> 21;
    s12 += carry11;
    s11 -= carry11 << 21;

    s0 += s12 * 666643;
    s1 += s12 * 470296;
    s2 += s12 * 654183;
    s3 -= s12 * 997805;
    s4 += s12 * 136657;
    s5 -= s12 * 683901;

    carry0 = s0 >> 21;
    s1 += carry0;
    s0 -= carry0 << 21;
    carry1 = s1 >> 21;
    s2 += carry1;
    s1 -= carry1 << 21;
    carry2 = s2 >> 21;
    s3 += carry2;
    s2 -= carry2 << 21;
    carry3 = s3 >> 21;
    s4 += carry3;
    s3 -= carry3 << 21;
    carry4 = s4 >> 21;
    s5 += carry4;
    s4 -= carry4 << 21;
    carry5 = s5 >> 21;
    s6 += carry5;
    s5 -= carry5 << 21;
    carry6 = s6 >> 21;
    s7 += carry6;
    s6 -= carry6 << 21;
    carry7 = s7 >> 21;
    s8 += carry7;
    s7 -= carry7 << 21;
    carry8 = s8 >> 21;
    s9 += carry8;
    s8 -= carry8 << 21;
    carry9 = s9 >> 21;
    s10 += carry9;
    s9 -= carry9 << 21;
    carry10 = s10 >> 21;
    s11 += carry10;
    s10 -= carry10 << 21;

    s[0] = (s0 >> 0) as u8;
    s[1] = (s0 >> 8) as u8;
    s[2] = ((s0 >> 16) | (s1 << 5)) as u8;
    s[3] = (s1 >> 3) as u8;
    s[4] = (s1 >> 11) as u8;
    s[5] = ((s1 >> 19) | (s2 << 2)) as u8;
    s[6] = (s2 >> 6) as u8;
    s[7] = ((s2 >> 14) | (s3 << 7)) as u8;
    s[8] = (s3 >> 1) as u8;
    s[9] = (s3 >> 9) as u8;
    s[10] = ((s3 >> 17) | (s4 << 4)) as u8;
    s[11] = (s4 >> 4) as u8;
    s[12] = (s4 >> 12) as u8;
    s[13] = ((s4 >> 20) | (s5 << 1)) as u8;
    s[14] = (s5 >> 7) as u8;
    s[15] = ((s5 >> 15) | (s6 << 6)) as u8;
    s[16] = (s6 >> 2) as u8;
    s[17] = (s6 >> 10) as u8;
    s[18] = ((s6 >> 18) | (s7 << 3)) as u8;
    s[19] = (s7 >> 5) as u8;
    s[20] = (s7 >> 13) as u8;
    s[21] = (s8 >> 0) as u8;
    s[22] = (s8 >> 8) as u8;
    s[23] = ((s8 >> 16) | (s9 << 5)) as u8;
    s[24] = (s9 >> 3) as u8;
    s[25] = (s9 >> 11) as u8;
    s[26] = ((s9 >> 19) | (s10 << 2)) as u8;
    s[27] = (s10 >> 6) as u8;
    s[28] = ((s10 >> 14) | (s11 << 7)) as u8;
    s[29] = (s11 >> 1) as u8;
    s[30] = (s11 >> 9) as u8;
    s[31] = (s11 >> 17) as u8;
}

#[cfg(feature = "blind-keys")]
pub fn sc_mul(a: &[u8], b: &[u8]) -> [u8; 32] {
    let mut s = [0u8; 32];
    sc_muladd(&mut s, a, b, &[0; 32]);
    s
}

#[cfg(feature = "blind-keys")]
pub fn sc_sq(s: &[u8]) -> [u8; 32] {
    sc_mul(s, s)
}

#[cfg(feature = "blind-keys")]
pub fn sc_sqmul(s: &[u8], n: usize, a: &[u8]) -> [u8; 32] {
    let mut t = [0u8; 32];
    t.copy_from_slice(s);
    for _ in 0..n {
        t = sc_sq(&t);
    }
    sc_mul(&t, a)
}

#[cfg(feature = "blind-keys")]
pub fn sc_invert(s: &[u8; 32]) -> [u8; 32] {
    let _10 = sc_sq(s);
    let _11 = sc_mul(s, &_10);
    let _100 = sc_mul(s, &_11);
    let _1000 = sc_sq(&_100);
    let _1010 = sc_mul(&_10, &_1000);
    let _1011 = sc_mul(s, &_1010);
    let _10000 = sc_sq(&_1000);
    let _10110 = sc_sq(&_1011);
    let _100000 = sc_mul(&_1010, &_10110);
    let _100110 = sc_mul(&_10000, &_10110);
    let _1000000 = sc_sq(&_100000);
    let _1010000 = sc_mul(&_10000, &_1000000);
    let _1010011 = sc_mul(&_11, &_1010000);
    let _1100011 = sc_mul(&_10000, &_1010011);
    let _1100111 = sc_mul(&_100, &_1100011);
    let _1101011 = sc_mul(&_100, &_1100111);
    let _10010011 = sc_mul(&_1000000, &_1010011);
    let _10010111 = sc_mul(&_100, &_10010011);
    let _10111101 = sc_mul(&_100110, &_10010111);
    let _11010011 = sc_mul(&_10110, &_10111101);
    let _11100111 = sc_mul(&_1010000, &_10010111);
    let _11101011 = sc_mul(&_100, &_11100111);
    let _11110101 = sc_mul(&_1010, &_11101011);

    let mut recip = sc_mul(&_1011, &_11110101);
    recip = sc_sqmul(&recip, 126, &_1010011);
    recip = sc_sqmul(&recip, 9, &_10);
    recip = sc_mul(&recip, &_11110101);
    recip = sc_sqmul(&recip, 7, &_1100111);
    recip = sc_sqmul(&recip, 9, &_11110101);
    recip = sc_sqmul(&recip, 11, &_10111101);
    recip = sc_sqmul(&recip, 8, &_11100111);
    recip = sc_sqmul(&recip, 9, &_1101011);
    recip = sc_sqmul(&recip, 6, &_1011);
    recip = sc_sqmul(&recip, 14, &_10010011);
    recip = sc_sqmul(&recip, 10, &_1100011);
    recip = sc_sqmul(&recip, 9, &_10010111);
    recip = sc_sqmul(&recip, 10, &_11110101);
    recip = sc_sqmul(&recip, 8, &_11010011);
    recip = sc_sqmul(&recip, 8, &_11101011);
    recip
}

pub fn sc_muladd(s: &mut [u8], a: &[u8], b: &[u8], c: &[u8]) {
    let a0 = 2097151 & load_3i(&a[0..3]);
    let a1 = 2097151 & (load_4i(&a[2..6]) >> 5);
    let a2 = 2097151 & (load_3i(&a[5..8]) >> 2);
    let a3 = 2097151 & (load_4i(&a[7..11]) >> 7);
    let a4 = 2097151 & (load_4i(&a[10..14]) >> 4);
    let a5 = 2097151 & (load_3i(&a[13..16]) >> 1);
    let a6 = 2097151 & (load_4i(&a[15..19]) >> 6);
    let a7 = 2097151 & (load_3i(&a[18..21]) >> 3);
    let a8 = 2097151 & load_3i(&a[21..24]);
    let a9 = 2097151 & (load_4i(&a[23..27]) >> 5);
    let a10 = 2097151 & (load_3i(&a[26..29]) >> 2);
    let a11 = load_4i(&a[28..32]) >> 7;
    let b0 = 2097151 & load_3i(&b[0..3]);
    let b1 = 2097151 & (load_4i(&b[2..6]) >> 5);
    let b2 = 2097151 & (load_3i(&b[5..8]) >> 2);
    let b3 = 2097151 & (load_4i(&b[7..11]) >> 7);
    let b4 = 2097151 & (load_4i(&b[10..14]) >> 4);
    let b5 = 2097151 & (load_3i(&b[13..16]) >> 1);
    let b6 = 2097151 & (load_4i(&b[15..19]) >> 6);
    let b7 = 2097151 & (load_3i(&b[18..21]) >> 3);
    let b8 = 2097151 & load_3i(&b[21..24]);
    let b9 = 2097151 & (load_4i(&b[23..27]) >> 5);
    let b10 = 2097151 & (load_3i(&b[26..29]) >> 2);
    let b11 = load_4i(&b[28..32]) >> 7;
    let c0 = 2097151 & load_3i(&c[0..3]);
    let c1 = 2097151 & (load_4i(&c[2..6]) >> 5);
    let c2 = 2097151 & (load_3i(&c[5..8]) >> 2);
    let c3 = 2097151 & (load_4i(&c[7..11]) >> 7);
    let c4 = 2097151 & (load_4i(&c[10..14]) >> 4);
    let c5 = 2097151 & (load_3i(&c[13..16]) >> 1);
    let c6 = 2097151 & (load_4i(&c[15..19]) >> 6);
    let c7 = 2097151 & (load_3i(&c[18..21]) >> 3);
    let c8 = 2097151 & load_3i(&c[21..24]);
    let c9 = 2097151 & (load_4i(&c[23..27]) >> 5);
    let c10 = 2097151 & (load_3i(&c[26..29]) >> 2);
    let c11 = load_4i(&c[28..32]) >> 7;

    let mut s0: i64 = c0 + a0 * b0;
    let mut s1: i64 = c1 + a0 * b1 + a1 * b0;
    let mut s2: i64 = c2 + a0 * b2 + a1 * b1 + a2 * b0;
    let mut s3: i64 = c3 + a0 * b3 + a1 * b2 + a2 * b1 + a3 * b0;
    let mut s4: i64 = c4 + a0 * b4 + a1 * b3 + a2 * b2 + a3 * b1 + a4 * b0;
    let mut s5: i64 = c5 + a0 * b5 + a1 * b4 + a2 * b3 + a3 * b2 + a4 * b1 + a5 * b0;
    let mut s6: i64 = c6 + a0 * b6 + a1 * b5 + a2 * b4 + a3 * b3 + a4 * b2 + a5 * b1 + a6 * b0;
    let mut s7: i64 =
        c7 + a0 * b7 + a1 * b6 + a2 * b5 + a3 * b4 + a4 * b3 + a5 * b2 + a6 * b1 + a7 * b0;
    let mut s8: i64 = c8
        + a0 * b8
        + a1 * b7
        + a2 * b6
        + a3 * b5
        + a4 * b4
        + a5 * b3
        + a6 * b2
        + a7 * b1
        + a8 * b0;
    let mut s9: i64 = c9
        + a0 * b9
        + a1 * b8
        + a2 * b7
        + a3 * b6
        + a4 * b5
        + a5 * b4
        + a6 * b3
        + a7 * b2
        + a8 * b1
        + a9 * b0;
    let mut s10: i64 = c10
        + a0 * b10
        + a1 * b9
        + a2 * b8
        + a3 * b7
        + a4 * b6
        + a5 * b5
        + a6 * b4
        + a7 * b3
        + a8 * b2
        + a9 * b1
        + a10 * b0;
    let mut s11: i64 = c11
        + a0 * b11
        + a1 * b10
        + a2 * b9
        + a3 * b8
        + a4 * b7
        + a5 * b6
        + a6 * b5
        + a7 * b4
        + a8 * b3
        + a9 * b2
        + a10 * b1
        + a11 * b0;
    let mut s12: i64 = a1 * b11
        + a2 * b10
        + a3 * b9
        + a4 * b8
        + a5 * b7
        + a6 * b6
        + a7 * b5
        + a8 * b4
        + a9 * b3
        + a10 * b2
        + a11 * b1;
    let mut s13: i64 = a2 * b11
        + a3 * b10
        + a4 * b9
        + a5 * b8
        + a6 * b7
        + a7 * b6
        + a8 * b5
        + a9 * b4
        + a10 * b3
        + a11 * b2;
    let mut s14: i64 =
        a3 * b11 + a4 * b10 + a5 * b9 + a6 * b8 + a7 * b7 + a8 * b6 + a9 * b5 + a10 * b4 + a11 * b3;
    let mut s15: i64 =
        a4 * b11 + a5 * b10 + a6 * b9 + a7 * b8 + a8 * b7 + a9 * b6 + a10 * b5 + a11 * b4;
    let mut s16: i64 = a5 * b11 + a6 * b10 + a7 * b9 + a8 * b8 + a9 * b7 + a10 * b6 + a11 * b5;
    let mut s17: i64 = a6 * b11 + a7 * b10 + a8 * b9 + a9 * b8 + a10 * b7 + a11 * b6;
    let mut s18: i64 = a7 * b11 + a8 * b10 + a9 * b9 + a10 * b8 + a11 * b7;
    let mut s19: i64 = a8 * b11 + a9 * b10 + a10 * b9 + a11 * b8;
    let mut s20: i64 = a9 * b11 + a10 * b10 + a11 * b9;
    let mut s21: i64 = a10 * b11 + a11 * b10;
    let mut s22: i64 = a11 * b11;
    let mut s23: i64 = 0;

    let mut carry0: i64 = (s0 + (1 << 20)) >> 21;
    s1 += carry0;
    s0 -= carry0 << 21;
    let mut carry2: i64 = (s2 + (1 << 20)) >> 21;
    s3 += carry2;
    s2 -= carry2 << 21;
    let mut carry4: i64 = (s4 + (1 << 20)) >> 21;
    s5 += carry4;
    s4 -= carry4 << 21;
    let mut carry6: i64 = (s6 + (1 << 20)) >> 21;
    s7 += carry6;
    s6 -= carry6 << 21;
    let mut carry8: i64 = (s8 + (1 << 20)) >> 21;
    s9 += carry8;
    s8 -= carry8 << 21;
    let mut carry10: i64 = (s10 + (1 << 20)) >> 21;
    s11 += carry10;
    s10 -= carry10 << 21;
    let mut carry12: i64 = (s12 + (1 << 20)) >> 21;
    s13 += carry12;
    s12 -= carry12 << 21;
    let mut carry14: i64 = (s14 + (1 << 20)) >> 21;
    s15 += carry14;
    s14 -= carry14 << 21;
    let mut carry16: i64 = (s16 + (1 << 20)) >> 21;
    s17 += carry16;
    s16 -= carry16 << 21;
    let carry18: i64 = (s18 + (1 << 20)) >> 21;
    s19 += carry18;
    s18 -= carry18 << 21;
    let carry20: i64 = (s20 + (1 << 20)) >> 21;
    s21 += carry20;
    s20 -= carry20 << 21;
    let carry22: i64 = (s22 + (1 << 20)) >> 21;
    s23 += carry22;
    s22 -= carry22 << 21;

    let mut carry1: i64 = (s1 + (1 << 20)) >> 21;
    s2 += carry1;
    s1 -= carry1 << 21;
    let mut carry3: i64 = (s3 + (1 << 20)) >> 21;
    s4 += carry3;
    s3 -= carry3 << 21;
    let mut carry5: i64 = (s5 + (1 << 20)) >> 21;
    s6 += carry5;
    s5 -= carry5 << 21;
    let mut carry7: i64 = (s7 + (1 << 20)) >> 21;
    s8 += carry7;
    s7 -= carry7 << 21;
    let mut carry9: i64 = (s9 + (1 << 20)) >> 21;
    s10 += carry9;
    s9 -= carry9 << 21;
    let mut carry11: i64 = (s11 + (1 << 20)) >> 21;
    s12 += carry11;
    s11 -= carry11 << 21;
    let mut carry13: i64 = (s13 + (1 << 20)) >> 21;
    s14 += carry13;
    s13 -= carry13 << 21;
    let mut carry15: i64 = (s15 + (1 << 20)) >> 21;
    s16 += carry15;
    s15 -= carry15 << 21;
    let carry17: i64 = (s17 + (1 << 20)) >> 21;
    s18 += carry17;
    s17 -= carry17 << 21;
    let carry19: i64 = (s19 + (1 << 20)) >> 21;
    s20 += carry19;
    s19 -= carry19 << 21;
    let carry21: i64 = (s21 + (1 << 20)) >> 21;
    s22 += carry21;
    s21 -= carry21 << 21;

    s11 += s23 * 666643;
    s12 += s23 * 470296;
    s13 += s23 * 654183;
    s14 -= s23 * 997805;
    s15 += s23 * 136657;
    s16 -= s23 * 683901;

    s10 += s22 * 666643;
    s11 += s22 * 470296;
    s12 += s22 * 654183;
    s13 -= s22 * 997805;
    s14 += s22 * 136657;
    s15 -= s22 * 683901;

    s9 += s21 * 666643;
    s10 += s21 * 470296;
    s11 += s21 * 654183;
    s12 -= s21 * 997805;
    s13 += s21 * 136657;
    s14 -= s21 * 683901;

    s8 += s20 * 666643;
    s9 += s20 * 470296;
    s10 += s20 * 654183;
    s11 -= s20 * 997805;
    s12 += s20 * 136657;
    s13 -= s20 * 683901;

    s7 += s19 * 666643;
    s8 += s19 * 470296;
    s9 += s19 * 654183;
    s10 -= s19 * 997805;
    s11 += s19 * 136657;
    s12 -= s19 * 683901;

    s6 += s18 * 666643;
    s7 += s18 * 470296;
    s8 += s18 * 654183;
    s9 -= s18 * 997805;
    s10 += s18 * 136657;
    s11 -= s18 * 683901;

    carry6 = (s6 + (1 << 20)) >> 21;
    s7 += carry6;
    s6 -= carry6 << 21;
    carry8 = (s8 + (1 << 20)) >> 21;
    s9 += carry8;
    s8 -= carry8 << 21;
    carry10 = (s10 + (1 << 20)) >> 21;
    s11 += carry10;
    s10 -= carry10 << 21;
    carry12 = (s12 + (1 << 20)) >> 21;
    s13 += carry12;
    s12 -= carry12 << 21;
    carry14 = (s14 + (1 << 20)) >> 21;
    s15 += carry14;
    s14 -= carry14 << 21;
    carry16 = (s16 + (1 << 20)) >> 21;
    s17 += carry16;
    s16 -= carry16 << 21;

    carry7 = (s7 + (1 << 20)) >> 21;
    s8 += carry7;
    s7 -= carry7 << 21;
    carry9 = (s9 + (1 << 20)) >> 21;
    s10 += carry9;
    s9 -= carry9 << 21;
    carry11 = (s11 + (1 << 20)) >> 21;
    s12 += carry11;
    s11 -= carry11 << 21;
    carry13 = (s13 + (1 << 20)) >> 21;
    s14 += carry13;
    s13 -= carry13 << 21;
    carry15 = (s15 + (1 << 20)) >> 21;
    s16 += carry15;
    s15 -= carry15 << 21;

    s5 += s17 * 666643;
    s6 += s17 * 470296;
    s7 += s17 * 654183;
    s8 -= s17 * 997805;
    s9 += s17 * 136657;
    s10 -= s17 * 683901;

    s4 += s16 * 666643;
    s5 += s16 * 470296;
    s6 += s16 * 654183;
    s7 -= s16 * 997805;
    s8 += s16 * 136657;
    s9 -= s16 * 683901;

    s3 += s15 * 666643;
    s4 += s15 * 470296;
    s5 += s15 * 654183;
    s6 -= s15 * 997805;
    s7 += s15 * 136657;
    s8 -= s15 * 683901;

    s2 += s14 * 666643;
    s3 += s14 * 470296;
    s4 += s14 * 654183;
    s5 -= s14 * 997805;
    s6 += s14 * 136657;
    s7 -= s14 * 683901;

    s1 += s13 * 666643;
    s2 += s13 * 470296;
    s3 += s13 * 654183;
    s4 -= s13 * 997805;
    s5 += s13 * 136657;
    s6 -= s13 * 683901;

    s0 += s12 * 666643;
    s1 += s12 * 470296;
    s2 += s12 * 654183;
    s3 -= s12 * 997805;
    s4 += s12 * 136657;
    s5 -= s12 * 683901;
    s12 = 0;

    carry0 = (s0 + (1 << 20)) >> 21;
    s1 += carry0;
    s0 -= carry0 << 21;
    carry2 = (s2 + (1 << 20)) >> 21;
    s3 += carry2;
    s2 -= carry2 << 21;
    carry4 = (s4 + (1 << 20)) >> 21;
    s5 += carry4;
    s4 -= carry4 << 21;
    carry6 = (s6 + (1 << 20)) >> 21;
    s7 += carry6;
    s6 -= carry6 << 21;
    carry8 = (s8 + (1 << 20)) >> 21;
    s9 += carry8;
    s8 -= carry8 << 21;
    carry10 = (s10 + (1 << 20)) >> 21;
    s11 += carry10;
    s10 -= carry10 << 21;

    carry1 = (s1 + (1 << 20)) >> 21;
    s2 += carry1;
    s1 -= carry1 << 21;
    carry3 = (s3 + (1 << 20)) >> 21;
    s4 += carry3;
    s3 -= carry3 << 21;
    carry5 = (s5 + (1 << 20)) >> 21;
    s6 += carry5;
    s5 -= carry5 << 21;
    carry7 = (s7 + (1 << 20)) >> 21;
    s8 += carry7;
    s7 -= carry7 << 21;
    carry9 = (s9 + (1 << 20)) >> 21;
    s10 += carry9;
    s9 -= carry9 << 21;
    carry11 = (s11 + (1 << 20)) >> 21;
    s12 += carry11;
    s11 -= carry11 << 21;

    s0 += s12 * 666643;
    s1 += s12 * 470296;
    s2 += s12 * 654183;
    s3 -= s12 * 997805;
    s4 += s12 * 136657;
    s5 -= s12 * 683901;
    s12 = 0;

    carry0 = s0 >> 21;
    s1 += carry0;
    s0 -= carry0 << 21;
    carry1 = s1 >> 21;
    s2 += carry1;
    s1 -= carry1 << 21;
    carry2 = s2 >> 21;
    s3 += carry2;
    s2 -= carry2 << 21;
    carry3 = s3 >> 21;
    s4 += carry3;
    s3 -= carry3 << 21;
    carry4 = s4 >> 21;
    s5 += carry4;
    s4 -= carry4 << 21;
    carry5 = s5 >> 21;
    s6 += carry5;
    s5 -= carry5 << 21;
    carry6 = s6 >> 21;
    s7 += carry6;
    s6 -= carry6 << 21;
    carry7 = s7 >> 21;
    s8 += carry7;
    s7 -= carry7 << 21;
    carry8 = s8 >> 21;
    s9 += carry8;
    s8 -= carry8 << 21;
    carry9 = s9 >> 21;
    s10 += carry9;
    s9 -= carry9 << 21;
    carry10 = s10 >> 21;
    s11 += carry10;
    s10 -= carry10 << 21;
    carry11 = s11 >> 21;
    s12 += carry11;
    s11 -= carry11 << 21;

    s0 += s12 * 666643;
    s1 += s12 * 470296;
    s2 += s12 * 654183;
    s3 -= s12 * 997805;
    s4 += s12 * 136657;
    s5 -= s12 * 683901;

    carry0 = s0 >> 21;
    s1 += carry0;
    s0 -= carry0 << 21;
    carry1 = s1 >> 21;
    s2 += carry1;
    s1 -= carry1 << 21;
    carry2 = s2 >> 21;
    s3 += carry2;
    s2 -= carry2 << 21;
    carry3 = s3 >> 21;
    s4 += carry3;
    s3 -= carry3 << 21;
    carry4 = s4 >> 21;
    s5 += carry4;
    s4 -= carry4 << 21;
    carry5 = s5 >> 21;
    s6 += carry5;
    s5 -= carry5 << 21;
    carry6 = s6 >> 21;
    s7 += carry6;
    s6 -= carry6 << 21;
    carry7 = s7 >> 21;
    s8 += carry7;
    s7 -= carry7 << 21;
    carry8 = s8 >> 21;
    s9 += carry8;
    s8 -= carry8 << 21;
    carry9 = s9 >> 21;
    s10 += carry9;
    s9 -= carry9 << 21;
    carry10 = s10 >> 21;
    s11 += carry10;
    s10 -= carry10 << 21;

    s[0] = (s0 >> 0) as u8;
    s[1] = (s0 >> 8) as u8;
    s[2] = ((s0 >> 16) | (s1 << 5)) as u8;
    s[3] = (s1 >> 3) as u8;
    s[4] = (s1 >> 11) as u8;
    s[5] = ((s1 >> 19) | (s2 << 2)) as u8;
    s[6] = (s2 >> 6) as u8;
    s[7] = ((s2 >> 14) | (s3 << 7)) as u8;
    s[8] = (s3 >> 1) as u8;
    s[9] = (s3 >> 9) as u8;
    s[10] = ((s3 >> 17) | (s4 << 4)) as u8;
    s[11] = (s4 >> 4) as u8;
    s[12] = (s4 >> 12) as u8;
    s[13] = ((s4 >> 20) | (s5 << 1)) as u8;
    s[14] = (s5 >> 7) as u8;
    s[15] = ((s5 >> 15) | (s6 << 6)) as u8;
    s[16] = (s6 >> 2) as u8;
    s[17] = (s6 >> 10) as u8;
    s[18] = ((s6 >> 18) | (s7 << 3)) as u8;
    s[19] = (s7 >> 5) as u8;
    s[20] = (s7 >> 13) as u8;
    s[21] = (s8 >> 0) as u8;
    s[22] = (s8 >> 8) as u8;
    s[23] = ((s8 >> 16) | (s9 << 5)) as u8;
    s[24] = (s9 >> 3) as u8;
    s[25] = (s9 >> 11) as u8;
    s[26] = ((s9 >> 19) | (s10 << 2)) as u8;
    s[27] = (s10 >> 6) as u8;
    s[28] = ((s10 >> 14) | (s11 << 7)) as u8;
    s[29] = (s11 >> 1) as u8;
    s[30] = (s11 >> 9) as u8;
    s[31] = (s11 >> 17) as u8;
}

pub fn sc_reject_noncanonical(s: &[u8]) -> Result<(), Error> {
    static L: [u8; 32] = [
        0xed, 0xd3, 0xf5, 0x5c, 0x1a, 0x63, 0x12, 0x58, 0xd6, 0x9c, 0xf7, 0xa2, 0xde, 0xf9, 0xde,
        0x14, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x10,
    ];
    if s.len() != 32 {
        panic!("Invalid compressed length")
    }
    let mut c: u8 = 0;
    let mut n: u8 = 1;

    let mut i = 31;
    loop {
        c |= ((((s[i] as i32) - (L[i] as i32)) >> 8) as u8) & n;
        n &= ((((s[i] ^ L[i]) as i32) - 1) >> 8) as u8;
        if i == 0 {
            break;
        }
        i -= 1;
    }
    if c != 0 {
        Ok(())
    } else {
        Err(Error::NonCanonical)
    }
}

static BI: [GePrecomp; 8] = [
    GePrecomp {
        y_plus_x: Fe([
            1288382639258501,
            245678601348599,
            269427782077623,
            1462984067271730,
            137412439391563,
        ]),
        y_minus_x: Fe([
            62697248952638,
            204681361388450,
            631292143396476,
            338455783676468,
            1213667448819585,
        ]),
        xy2d: Fe([
            301289933810280,
            1259582250014073,
            1422107436869536,
            796239922652654,
            1953934009299142,
        ]),
    },
    GePrecomp {
        y_plus_x: Fe([
            1601611775252272,
            1720807796594148,
            1132070835939856,
            1260455018889551,
            2147779492816911,
        ]),
        y_minus_x: Fe([
            316559037616741,
            2177824224946892,
            1459442586438991,
            1461528397712656,
            751590696113597,
        ]),
        xy2d: Fe([
            1850748884277385,
            1200145853858453,
            1068094770532492,
            672251375690438,
            1586055907191707,
        ]),
    },
    GePrecomp {
        y_plus_x: Fe([
            769950342298419,
            132954430919746,
            844085933195555,
            974092374476333,
            726076285546016,
        ]),
        y_minus_x: Fe([
            425251763115706,
            608463272472562,
            442562545713235,
            837766094556764,
            374555092627893,
        ]),
        xy2d: Fe([
            1086255230780037,
            274979815921559,
            1960002765731872,
            929474102396301,
            1190409889297339,
        ]),
    },
    GePrecomp {
        y_plus_x: Fe([
            665000864555967,
            2065379846933859,
            370231110385876,
            350988370788628,
            1233371373142985,
        ]),
        y_minus_x: Fe([
            2019367628972465,
            676711900706637,
            110710997811333,
            1108646842542025,
            517791959672113,
        ]),
        xy2d: Fe([
            965130719900578,
            247011430587952,
            526356006571389,
            91986625355052,
            2157223321444601,
        ]),
    },
    GePrecomp {
        y_plus_x: Fe([
            1802695059465007,
            1664899123557221,
            593559490740857,
            2160434469266659,
            927570450755031,
        ]),
        y_minus_x: Fe([
            1725674970513508,
            1933645953859181,
            1542344539275782,
            1767788773573747,
            1297447965928905,
        ]),
        xy2d: Fe([
            1381809363726107,
            1430341051343062,
            2061843536018959,
            1551778050872521,
            2036394857967624,
        ]),
    },
    GePrecomp {
        y_plus_x: Fe([
            1970894096313054,
            528066325833207,
            1619374932191227,
            2207306624415883,
            1169170329061080,
        ]),
        y_minus_x: Fe([
            2070390218572616,
            1458919061857835,
            624171843017421,
            1055332792707765,
            433987520732508,
        ]),
        xy2d: Fe([
            893653801273833,
            1168026499324677,
            1242553501121234,
            1306366254304474,
            1086752658510815,
        ]),
    },
    GePrecomp {
        y_plus_x: Fe([
            213454002618221,
            939771523987438,
            1159882208056014,
            317388369627517,
            621213314200687,
        ]),
        y_minus_x: Fe([
            1971678598905747,
            338026507889165,
            762398079972271,
            655096486107477,
            42299032696322,
        ]),
        xy2d: Fe([
            177130678690680,
            1754759263300204,
            1864311296286618,
            1180675631479880,
            1292726903152791,
        ]),
    },
    GePrecomp {
        y_plus_x: Fe([
            1913163449625248,
            460779200291993,
            2193883288642314,
            1008900146920800,
            1721983679009502,
        ]),
        y_minus_x: Fe([
            1070401523076875,
            1272492007800961,
            1910153608563310,
            2075579521696771,
            1191169788841221,
        ]),
        xy2d: Fe([
            692896803108118,
            500174642072499,
            2068223309439677,
            1162190621851337,
            1426986007309901,
        ]),
    },
];

#[cfg(test)]
pub(crate) fn small_order_points() -> [GeP3; 8] {
    let order_8_point = GeP3::from_bytes_vartime(&[
        0x26, 0xe8, 0x95, 0x8f, 0xc2, 0xb2, 0x27, 0xb0, 0x45, 0xc3, 0xf4, 0x89, 0xf2, 0xef, 0x98,
        0xf0, 0xd5, 0xdf, 0xac, 0x05, 0xd3, 0xc6, 0x33, 0x39, 0xb1, 0x38, 0x02, 0x88, 0x6d, 0x53,
        0xfc, 0x05,
    ])
    .unwrap();
    let mut points = [order_8_point; 8];
    for i in 1..8 {
        points[i] = points[i - 1] + order_8_point;
    }
    points
}

#[test]
fn test_has_small_order() {
    let mut scalar = [0u8; 32];
    scalar[0] = 42;
    let p = ge_scalarmult_base(&scalar);
    assert!(!p.has_small_order());

    let points = small_order_points();
    for (i, t) in points.iter().enumerate() {
        let encoded = t.to_bytes();
        assert!(points[..i].iter().all(|q| q.to_bytes() != encoded));
        assert!(t.has_small_order());
        assert!(GeP3::from_bytes_negate_vartime(&encoded)
            .unwrap()
            .has_small_order());
        // A point with a small-order part isn't small-order itself.
        assert!(!(p + *t).has_small_order());
    }
}
