// The help texts of MME's Settings window (the "?" buttons), in our own words.

export const MODEL_HELP: { title: string; text: string }[] = [
  {
    title: "Model A",
    text: "The first model, built from a small set of inputs. Sometimes more accurate than B.",
  },
  {
    title: "Model B",
    text: "Built second with more information about the lair and the route.",
  },
  {
    title: "Model C",
    text: "Adds first-round and minimum-round damage to the inputs, so it is closer than an average alone.",
  },
  {
    title: "Model D",
    text: "A round-by-round simulation tuned from C to fix its weaknesses.",
  },
];

export const MODEL_NOTES = [
  "Every ticked model runs and the results are averaged.",
  "No recovery: average the chosen models for kill time and movement, but only apply the standard filter for incoming damage, so no recovery time is counted.",
  "Show in detail: list each model's estimate beside the average in a monster's detail pane.",
];

export interface KnobHelp {
  key: "knob_xp" | "knob_dmg" | "knob_mana" | "knob_move";
  label: string;
  text: string;
}

export const KNOB_HELP: KnobHelp[] = [
  {
    key: "knob_xp",
    label: "Exp",
    text: "Multiplies the exp/hr result just before it is returned.",
  },
  {
    key: "knob_dmg",
    label: "Damage",
    text: "Scales incoming damage. Larger makes monsters hurt more, and so adds recovery time; smaller makes them hurt less.",
  },
  {
    key: "knob_mana",
    label: "Mana",
    text: "Scales mana recovery. Smaller means less time spent recovering mana.",
  },
  {
    key: "knob_move",
    label: "Move",
    text: "Scales movement between lairs. Larger means more time spent moving.",
  },
];

export const CLUSTER_NOTE =
  "Model A cannot always tell a small cluster of lairs inside a large area (a gnoll camp, say). When its average walk is 2 or less and the lairs in the area, times the movement knob's factor, are still fewer than the rooms, it treats the area as a cluster and cuts the movement penalty.";
