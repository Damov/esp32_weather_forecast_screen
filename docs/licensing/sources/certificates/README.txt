Certificate source material (MPL-2.0)

The bundled cacrt_all.pem is the exact input to ESP-IDF's certificate-bundle
conversion. Its header preserves the Mozilla attribution, extraction date,
upstream source reference and conversion provenance. gen_crt_bundle.py and
sdkconfig.json describe the conversion and selection used by this firmware.
Modify the PEM input and rebuild the firmware to change its trust store.
These source materials remain available under MPL-2.0; the project's MIT
license does not replace their terms. No additional restriction is imposed
on recipients' rights to this material.

Mozilla original certificate data: https://hg.mozilla.org/projects/nss/file/tip/lib/ckfw/builtins/certdata.txt
PEM extraction tool and information: https://curl.se/docs/caextract.html
