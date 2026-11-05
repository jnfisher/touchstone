//! Enums for codes MME names in `modMMudFunc` (`GetItemTypeEnum`, `GetWornTypeEnum`, ...).
//! Tables keep the raw number; `from_code` returns `None` for codes MME reports as unknown.
use serde::Serialize;

macro_rules! code_enum {
    ($(#[$meta:meta])* $name:ident { $($var:ident = $label:literal : $($code:literal)|+),+ $(,)? }) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
        pub enum $name { $($var),+ }

        impl $name {
            /// The variant for a raw database code, or `None` if MME treats it as unknown.
            #[allow(clippy::manual_range_patterns)] // codes are listed as the MME source lists them
            pub fn from_code(code: i32) -> Option<Self> {
                match code {
                    $($($code)|+ => Some(Self::$var),)+
                    _ => None,
                }
            }

            /// The label MME displays for this code.
            pub fn label(self) -> &'static str {
                match self { $(Self::$var => $label),+ }
            }
        }
    };
}

code_enum! {
    /// `Items.ItemType` (port of modMMudFunc GetItemTypeEnum).
    ItemType {
        Armour = "Armour": 0, Weapon = "Weapon": 1, Projectile = "Projectile": 2,
        Sign = "Sign": 3, Food = "Food": 4, Drink = "Drink": 5, Light = "Light": 6,
        Key = "Key": 7, Container = "Container": 8, Scroll = "Scroll": 9, Special = "Special": 10,
    }
}

code_enum! {
    /// `Items.WeaponType` (port of modMMudFunc GetWeaponTypeEnum).
    WeaponType {
        OneHandBlunt = "1H Blunt": 0, TwoHandBlunt = "2H Blunt": 1,
        OneHandSharp = "1H Sharp": 2, TwoHandSharp = "2H Sharp": 3,
    }
}

code_enum! {
    /// `Classes.WeaponType` (port of modMMudFunc GetClassWeaponTypeEnum).
    ClassWeaponType {
        OneHandBlunt = "1H Blunt": 0, TwoHandBlunt = "2H Blunt": 1,
        OneHandSharp = "1H Sharp": 2, TwoHandSharp = "2H Sharp": 3,
        AnyOneHand = "Any 1H": 4, AnyTwoHand = "Any 2H": 5, AnySharp = "Any Sharp": 6,
        AnyBlunt = "Any Blunt": 7, AnyWeapon = "Any Weapon": 8, Staff = "Staff": 9,
    }
}

code_enum! {
    /// `Items.ArmourType` / `Classes.ArmourType` (port of GetArmourTypeEnum). MME folds codes 3
    /// to 6 (the four leather grades) into one label.
    ArmourType {
        Natural = "Natural": 0, Silk = "Silk": 1, Ninja = "Ninja": 2,
        Leather = "Leather": 3 | 4 | 5 | 6, Chainmail = "Chainmail": 7,
        Scalemail = "Scalemail": 8, Platemail = "Platemail": 9,
    }
}

code_enum! {
    /// `Items.Worn` (port of GetWornTypeEnum). Codes 4 and 13 are both Finger; 14 and 17 both
    /// Wrist.
    WornSlot {
        Nowhere = "Nowhere": 0, Everywhere = "Everywhere": 1, Head = "Head": 2,
        Hands = "Hands": 3, Finger = "Finger": 4 | 13, Feet = "Feet": 5, Arms = "Arms": 6,
        Back = "Back": 7, Neck = "Neck": 8, Legs = "Legs": 9, Waist = "Waist": 10,
        Torso = "Torso": 11, OffHand = "Off-Hand": 12, Wrist = "Wrist": 14 | 17,
        Ears = "Ears": 15, Worn = "Worn": 16, Eyes = "Eyes": 18, Face = "Face": 19,
    }
}

code_enum! {
    /// `Items.Currency` (port of GetCostTypeEnum).
    Currency {
        Copper = "Copper": 0, Silver = "Silver": 1, Gold = "Gold": 2, Platinum = "Platinum": 3,
        Runic = "Runic": 4,
    }
}

code_enum! {
    /// `Spells.Targets` (port of GetSpellTargetsEnum).
    SpellTargets {
        User = "User": 0, SelfOnly = "Self": 1, SelfOrUser = "Self or User": 2,
        DividedAreaNotSelf = "Divided Area (not self)": 3, Monster = "Monster": 4,
        DividedAreaInclSelf = "Divided Area (incl self)": 5, Any = "Any": 6, Item = "Item": 7,
        MonsterOrUser = "Monster or User": 8, DividedAttackArea = "Divided Attack Area": 9,
        DividedPartyArea = "Divided Party Area": 10, FullArea = "Full Area": 11,
        FullAttackArea = "Full Attack Area": 12, FullPartyArea = "Full Party Area": 13,
    }
}

code_enum! {
    /// `Spells.AttType` (port of SpellAttackTypeEnum).
    SpellAttackType {
        Cold = "Cold": 0, Fire = "Fire": 1, Stone = "Stone": 2, Lightning = "Lightning": 3,
        Normal = "Normal": 4, Water = "Water": 5, Poison = "Poison": 6,
    }
}

code_enum! {
    /// `Shops.ShopType` (port of GetShopTypeEnum).
    ShopType {
        General = "General": 0, Weapons = "Weapons": 1, Armour = "Armour": 2, Items = "Items": 3,
        Spells = "Spells": 4, Hospital = "Hospital": 5, Tavern = "Tavern": 6, Bank = "Bank": 7,
        Training = "Training": 8, Inn = "Inn": 9, Specific = "Specific": 10,
        GangShop = "Gang Shop": 11, DeedShop = "Deed Shop": 12,
    }
}

code_enum! {
    /// `Monsters.AttType-n` (port of GetMonAttackTypeEnum).
    MonsterAttackType {
        None = "None": 0, Normal = "Normal": 1, Spell = "Spell": 2, Rob = "Rob": 3,
    }
}

code_enum! {
    /// `Monsters.Type` (port of GetMonTypeEnum).
    MonsterType {
        Solo = "Solo": 0, Leader = "Leader": 1, Follower = "Follower": 2,
        Stationary = "Stationary": 3,
    }
}

code_enum! {
    /// `Monsters.Align` (port of GetMonAlignmentEnum).
    Alignment {
        Good = "Good": 0, Evil = "Evil": 1, ChaoticEvil = "Chaotic Evil": 2, Neutral = "Neutral": 3,
        LawfulGood = "Lawful Good": 4, NeutralEvil = "Neutral Evil": 5, LawfulEvil = "Lawful Evil": 6,
    }
}

code_enum! {
    /// `Classes.MageryType` / `Spells.Magery` (port of GetMageryEnum).
    Magery {
        None = "None": 0, Mage = "Mage": 1, Priest = "Priest": 2, Druid = "Druid": 3,
        Bard = "Bard": 4, Kai = "Kai": 5,
    }
}
