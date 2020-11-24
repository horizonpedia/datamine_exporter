# Horizonpedia Datamine Exporter

This tool exports the [Animal Crossing: New Horizons datamine spreadsheet](https://docs.google.com/spreadsheets/d/13d_LAJPlxMa_DubPTuirkIV4DERBMXbrWQsmSh8ReK4) as json files.

# Prerequisites
You need to a working Rust installation and a Google API key.

## Installing Rust
See https://rustup.rs.

## Google API key
Go to https://console.developers.google.com, create a new project, add/enable the spreadsheet API for it and then create an API key (not OAUTH).
Create a file called `.env` in the project root and enter your API key there:

```
API_KEY=replace_me_with_your_api_key
```

# Running
Just execute this in the project root:
```
cargo run
```
  # Optional Parameters
  If you want to include the images in the download, execute:

    ```
    cargo run -- --dl-images
    ```

It will create a folder called `export`, containing all the sheets in json format.
