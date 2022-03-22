use anyhow::Result;
use dialoguer::{MultiSelect, theme::{Theme, ColorfulTheme}};

pub fn process_interactive() -> Result<()> {
    let items = vec!["Option 1", "Option 2"];
    let multiselected = &[
      "Ice Cream",
      "Vanilla Cupcake",
      "Chocolate Muffin",
      "A Pile of sweet, sweet mustard",
  ];
  let defaults = &[false, false, true, false];
  let selections = MultiSelect::with_theme(&ColorfulTheme::default())
      .with_prompt("Pick your food")
      .items(&multiselected[..])
      .defaults(&defaults[..])
      .interact()
      .unwrap();

    Ok(())
}
