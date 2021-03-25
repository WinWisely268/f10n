[![test](https://github.com/WinWisely268/f10n/actions/workflows/test.yml/badge.svg)](https://github.com/WinWisely268/f10n/actions/workflows/test.yml)

# F10N

A cli tool to translate arb template file to other languages (for flutter).

```
f10n 0.1
Alexander Adhyatma <alex@asiatech.dev>
translate template arb file to any language for flutter

USAGE:
f10n [OPTIONS]

FLAGS:
-h, --help       Prints help information
-V, --version    Prints version information

OPTIONS:
-t, --arb-template <arb-template>    path to the arb template [default: app_en.arb]
-c, --cache <cache>                  path to the cache file (json) [default: translations_cache.json]
-o, --output <prefix-dir>            prefix-dir for the output [default: ./testdata]
-l, --lang <target-languages>...     target languages separated by space i.e. fr de es [default: fr de es it tr]
```

## Setup

- You need to make sure you have a **Service Account** for your GCP project.
- To create it, create a new GCP project, and activate its billing.
- Enable Cloud Translation API.
- Create service account under IAM & Admin Menu in GCP
- Make sure the service account has role for accessing translation.
- Create the service key (new pair), and save the private one to your local disk.
- You need to make sure you have `$GOOGLE_APPLICATION_CREDENTIALS` environment variable is set to point to the location
  of your private key that was previously downloaded.
- Profit.

### Translation

The whole point of the application is to make sure we have a local cache (in this case a local embedded DB using `sled`)
, so that we don't call google every time and saves us some money.

## Example Usage

- do a `cd` to this repo
- do `f10n -t intl_messages.arb -c $HOME/.cache/translations -o examples -l en de fr it es tr ur id cn`
- see your translations.
