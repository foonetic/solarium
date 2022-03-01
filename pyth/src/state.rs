use crate::error::Result;
use crate::pack::PythPack;
use arrayref::{array_mut_ref, array_ref, array_refs, mut_array_refs};
use num_enum::{IntoPrimitive, TryFromPrimitive};

#[derive(Copy, Clone, Debug, PartialEq, Eq, TryFromPrimitive, IntoPrimitive)]
#[repr(u32)]
pub enum AccountType
{
  Unknown,
  Mapping,
  Product,
  Price
}

#[derive(Eq, PartialEq, PartialOrd, Debug, Copy, Clone, TryFromPrimitive, IntoPrimitive)]
#[repr(u32)]
pub enum PriceStatus {
    /// The price feed is not currently updating for an unknown reason.
    Unknown,
    /// The price feed is updating as expected.
    Trading,
    /// The price feed is not currently updating because trading in the product has been halted.
    Halted,
    /// The price feed is not currently updating because an auction is setting the price.
    Auction,
}

#[derive(Eq, PartialEq, PartialOrd, Debug, Copy, Clone, TryFromPrimitive, IntoPrimitive)]
#[repr(u32)]
pub enum CorpAction {
    NoCorpAct,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[repr(C)]
pub struct AccKey {
    pub val: [u8; 32],
}

#[derive(Eq, PartialEq, PartialOrd, Debug, Copy, Clone, TryFromPrimitive, IntoPrimitive)]
#[repr(u32)]
pub enum PriceType {
    Unknown,
    Price,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[repr(C)]
pub struct PriceInfo {
    /// the current price.
    /// For the aggregate price use price.get_current_price() whenever possible. It has more checks to make sure price is valid.
    pub price: i64,
    /// confidence interval around the price.
    /// For the aggregate confidence use price.get_current_price() whenever possible. It has more checks to make sure price is valid.
    pub conf: u64,
    /// status of price (Trading is valid).
    /// For the aggregate status use price.get_current_status() whenever possible.
    /// Price data can sometimes go stale and the function handles the status in such cases.
    pub status: PriceStatus,
    /// notification of any corporate action
    pub corp_act: CorpAction,
    pub pub_slot: u64,
}

impl PythPack for PriceInfo {
    const LEN: usize = 32;

    fn unpack_from_slice(src: &[u8]) -> Result<Self> {
        let src = array_ref![src, 0, PriceInfo::LEN];
        let (price, conf, status, corp_act, pub_slot) = array_refs![src, 8, 8, 4, 4, 8];
        let price = i64::from_le_bytes(*price);
        let conf = u64::from_le_bytes(*conf);
        let status = PriceStatus::try_from_primitive(u32::from_le_bytes(*status)).unwrap();
        let corp_act = CorpAction::try_from_primitive(u32::from_le_bytes(*corp_act)).unwrap();
        let pub_slot = u64::from_le_bytes(*pub_slot);

        Ok(Self {
            price,
            conf,
            status,
            corp_act,
            pub_slot,
        })
    }

    fn pack_into_slice(&self, dst: &mut [u8]) -> Result<()> {
        let dst = array_mut_ref![dst, 0, PriceInfo::LEN];
        let (price_dst, conf_dst, status_dst, corp_act_dst, pub_slot_dst) =
            mut_array_refs![dst, 8, 8, 4, 4, 8];
        *price_dst = self.price.to_le_bytes();
        *conf_dst = self.conf.to_le_bytes();

        let status_prim: u32 = self.status.try_into().unwrap();
        *status_dst = status_prim.to_le_bytes();

        let ca_prim: u32 = self.corp_act.try_into().unwrap();
        *corp_act_dst = ca_prim.to_le_bytes();

        *pub_slot_dst = self.pub_slot.to_le_bytes();

        Ok(())
    }
}

/// The price and confidence contributed by a specific publisher.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[repr(C)]
pub struct PriceComp {
    /// key of contributing publisher
    pub publisher: AccKey,
    /// the price used to compute the current aggregate price
    pub agg: PriceInfo,
    /// The publisher's latest price. This price will be incorporated into the aggregate price
    /// when price aggregation runs next.
    pub latest: PriceInfo,
}

impl PythPack for PriceComp {
    const LEN: usize = 96;

    fn unpack_from_slice(src: &[u8]) -> Result<Self> {
        let src = array_ref![src, 0, PriceComp::LEN];
        let (publisher, agg, latest) = array_refs![src, 32, PriceInfo::LEN, PriceInfo::LEN];
        let publisher = AccKey { val: *publisher };

        let agg = PriceInfo::unpack_from_slice(agg)?;
        let latest = PriceInfo::unpack_from_slice(latest)?;

        Ok(Self {
            publisher,
            agg,
            latest,
        })
    }

    fn pack_into_slice(&self, dst: &mut [u8]) -> Result<()> {
        let dst = array_mut_ref![dst, 0, PriceComp::LEN];
        let (pub_dst, agg_dst, latest_dst) =
            mut_array_refs![dst, 32, PriceInfo::LEN, PriceInfo::LEN];

        *pub_dst = self.publisher.val;

        self.agg.pack_into_slice(agg_dst)?;
        self.latest.pack_into_slice(latest_dst)?;

        Ok(())
    }
}

/// An exponentially-weighted moving average.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[repr(C)]
pub struct Ema {
    /// The current value of the EMA
    pub val: i64,
    /// numerator state for next update
    pub numer: i64,
    /// denominator state for next update
    pub denom: i64,
}

impl PythPack for Ema {
    const LEN: usize = 24;

    fn unpack_from_slice(src: &[u8]) -> Result<Self> {
        let src = array_ref![src, 0, Ema::LEN];
        let (val, numer, denom) = array_refs![src, 8, 8, 8];
        let val = i64::from_le_bytes(*val);
        let numer = i64::from_le_bytes(*numer);
        let denom = i64::from_le_bytes(*denom);

        Ok(Self { val, numer, denom })
    }

    fn pack_into_slice(&self, dst: &mut [u8]) -> Result<()> {
        let dst = array_mut_ref![dst, 0, Ema::LEN];
        let (val_dst, numer_dst, denom_dst) = mut_array_refs![dst, 8, 8, 8];
        *val_dst = self.val.to_le_bytes();
        *numer_dst = self.numer.to_le_bytes();
        *denom_dst = self.denom.to_le_bytes();

        Ok(())
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[repr(C)]
pub struct Price {
    /// pyth magic number
    pub magic: u32,
    /// program version
    pub ver: u32,
    /// account type
    pub atype: u32,
    /// price account size
    pub size: u32,
    /// price or calculation type
    pub ptype: PriceType,
    /// price exponent
    pub expo: i32,
    /// number of component prices
    pub num: u32,
    /// number of quoters that make up aggregate
    pub num_qt: u32,
    /// slot of last valid (not unknown) aggregate price
    pub last_slot: u64,
    /// valid slot-time of agg. price
    pub valid_slot: u64,
    /// time-weighted average price
    pub twap: Ema,
    /// time-weighted average confidence interval
    pub twac: Ema,
    /// space for future derived values
    pub drv1: i64,
    /// space for future derived values
    pub drv2: i64,
    /// product account key
    pub prod: AccKey,
    /// next Price account in linked list
    pub next: AccKey,
    /// valid slot of previous update
    pub prev_slot: u64,
    /// aggregate price of previous update
    pub prev_price: i64,
    /// confidence interval of previous update
    pub prev_conf: u64,
    /// space for future derived values
    pub drv3: i64,
    /// aggregate price info
    pub agg: PriceInfo,
    // pub comp: [PriceComp; 32], SIZE BREAKS STACKFRAME, NOT SUPPORTED
}

impl PythPack for Price {
    const LEN: usize = 240; 

    fn unpack_from_slice(src: &[u8]) -> Result<Self> {
        let src = array_ref![src, 0, Price::LEN];
        let (
            magic,
            ver,
            atype,
            size,
            ptype,
            expo,
            num,
            num_qt,
            last_slot,
            valid_slot,
            twap,
            twac,
            drv1,
            drv2,
            prod,
            next,
            prev_slot,
            prev_price,
            prev_conf,
            drv3,
            agg,
        ) = array_refs![
            src,
            4,
            4,
            4,
            4,
            4,
            4,
            4,
            4,
            8,
            8,
            Ema::LEN,
            Ema::LEN,
            8,
            8,
            32,
            32,
            8,
            8,
            8,
            8,
            PriceInfo::LEN
        ];
        let magic = u32::from_le_bytes(*magic);
        let ver = u32::from_le_bytes(*ver);
        let atype = u32::from_le_bytes(*atype);
        let size = u32::from_le_bytes(*size);
        let ptype = PriceType::try_from_primitive(u32::from_le_bytes(*ptype)).unwrap();
        let expo = i32::from_le_bytes(*expo);
        let num = u32::from_le_bytes(*num);
        let num_qt = u32::from_le_bytes(*num_qt);
        let last_slot = u64::from_le_bytes(*last_slot);
        let valid_slot = u64::from_le_bytes(*valid_slot);
        let twap = Ema::unpack_from_slice(twap)?;
        let twac = Ema::unpack_from_slice(twac)?;
        let drv1 = i64::from_le_bytes(*drv1);
        let drv2 = i64::from_le_bytes(*drv2);
        let prod = AccKey { val: *prod };

        let next = AccKey { val: *next };
        let prev_slot = u64::from_le_bytes(*prev_slot);
        let prev_price = i64::from_le_bytes(*prev_price);
        let prev_conf = u64::from_le_bytes(*prev_conf);
        let drv3 = i64::from_le_bytes(*drv3);
        let agg = PriceInfo::unpack_from_slice(agg)?;

        Ok(Self {
            magic,
            ver,
            atype,
            size,
            ptype,
            expo,
            num,
            num_qt,
            last_slot,
            valid_slot,
            twap,
            twac,
            drv1,
            drv2,
            prod,
            next,
            prev_slot,
            prev_price,
            prev_conf,
            drv3,
            agg,
        })
    }

    fn pack_into_slice(&self, dst: &mut [u8]) -> Result<()> {
        let dst = array_mut_ref![dst, 0, Price::LEN];
        let (
            magic_dst,
            ver_dst,
            atype_dst,
            size_dst,
            ptype_dst,
            expo_dst,
            num_dst,
            num_qt_dst,
            last_slot_dst,
            valid_slot_dst,
            twap_dst,
            twac_dst,
            drv1_dst,
            drv2_dst,
            prod_dst,
            next_dst,
            prev_slot_dst,
            prev_price_dst,
            prev_conf_dst,
            drv3_dst,
            agg_dst,
        ) = mut_array_refs![
            dst,
            4,
            4,
            4,
            4,
            4,
            4,
            4,
            4,
            8,
            8,
            Ema::LEN,
            Ema::LEN,
            8,
            8,
            32,
            32,
            8,
            8,
            8,
            8,
            PriceInfo::LEN
        ];

        *magic_dst = self.magic.to_le_bytes();
        *ver_dst = self.ver.to_le_bytes();
        *atype_dst = self.atype.to_le_bytes();
        *size_dst = self.size.to_le_bytes();

        let ptype_it: u32 = self.ptype.try_into().unwrap();
        *ptype_dst = ptype_it.to_le_bytes();

        *expo_dst = self.expo.to_le_bytes();
        *num_dst = self.num.to_le_bytes();
        *num_qt_dst = self.num_qt.to_le_bytes();
        *last_slot_dst = self.last_slot.to_le_bytes();
        *valid_slot_dst = self.valid_slot.to_le_bytes();

        self.twap.pack_into_slice(twap_dst)?;
        self.twac.pack_into_slice(twac_dst)?;

        *drv1_dst = self.drv1.to_le_bytes();
        *drv2_dst = self.drv2.to_le_bytes();

        *prod_dst = self.prod.val;
        *next_dst = self.next.val;

        *prev_slot_dst = self.prev_slot.to_le_bytes();
        *prev_price_dst = self.prev_price.to_le_bytes();
        *prev_conf_dst = self.prev_conf.to_le_bytes();

        *drv3_dst = self.drv3.to_le_bytes();

        self.agg.pack_into_slice(agg_dst)?;

        Ok(())
    }
}
