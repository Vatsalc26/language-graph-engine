# Foundation Test Vectors (Phase 1 & Phase 2)

This document locks in the golden vector values for canonical entities and their content-addressed CIDv1 identifiers in Phase 1 and Phase 2 of the **Language Graph Engine**.

Any change to these CIDs indicates a modification in the serialization structure or CID hashing mechanism, which constitutes a breaking migration event.

---

## 1. Golden Revision CIDs (Phase 1 & 2)

These are the deterministic CIDv1 identifiers generated for canonical seeded grapheme revisions using the schema `language-graph/grapheme-revision/v1`, encoded in canonical DAG-CBOR and hashed via SHA2-256.

| Character | Stable Entity URN | Golden Revision CIDv1 | Phase |
| --------- | ----------------- | --------------------- | ----- |
| **a** | `urn:language-graph:grapheme:nfc:0061` | `bafyreigzc6usxy4ufmz43vpotqo54cqrdqwtzgebsxwhyybmkgphbfwq5a` | Phase 1 |
| **z** | `urn:language-graph:grapheme:nfc:007a` | `bafyreiamiyi7szcus6c67balumi6ejdvby5jiad5yr375sveyufkkb63dm` | Phase 1 |
| **A** | `urn:language-graph:grapheme:nfc:0041` | `bafyreievw56s5ltwde2xmmxzt3etkfz73qsutx43x7xuxe3iuvnbpobm2e` | Phase 2 |
| **Z** | `urn:language-graph:grapheme:nfc:005a` | `bafyreiezwovnoifhtfjxgtotbszxaku5bkc4br7vdjopukdmxhtbbbju2y` | Phase 2 |
| **0** | `urn:language-graph:grapheme:nfc:0030` | `bafyreihed7ebhkvf5b27tyuhzwsdp3bcrjdxiwow2idla4rvw5qskorbxe` | Phase 2 |
| **9** | `urn:language-graph:grapheme:nfc:0039` | `bafyreihjhfma5buevawmzrchj3bauifuvc5ey5b2x4fgfsjmiqqease33u` | Phase 2 |
| **SPACE** | `urn:language-graph:grapheme:nfc:0020` | `bafyreife2nx5traghcw3frzjh4wo2rb2ww5cybvptdz6c3ayn2ukioagnq` | Phase 2 |
| **.** | `urn:language-graph:grapheme:nfc:002e` | `bafyreiecsu3larivauz7adpncch22ml26srmct2jvtfv3jc4w73bi3omvu` | Phase 2 |
| **'** | `urn:language-graph:grapheme:nfc:0027` | `bafyreia4iwm7sbr6rirxz5rhtn23h6awmrivwobujbgyyx66rqvdksqfam` | Phase 2 |
| **"** | `urn:language-graph:grapheme:nfc:0022` | `bafyreiemmlqku44pnvm2tbxlfuaocoypp4eqxukzgspw4zhyuglixyh6du` | Phase 2 |
| **!** | `urn:language-graph:grapheme:nfc:0021` | `bafyreieaz24s7nzw54rchufcv4btgmotic3j6tr7is4gbmj7ul74b57wdm` | Phase 2 |

---

## 2. Golden Snapshot CIDs

### Lowercase Alphabet Snapshot (Phase 1)
* **Collection URN**: `urn:language-graph:collection:latin-lowercase-a-z`
* **Golden Snapshot CIDv1**: `bafyreib4ivpoazb5skkr7yvfelvoowz6sxxncdsjewvxawyedm5tikeshm`

### Uppercase Alphabet Snapshot (Phase 2)
* **Collection URN**: `urn:language-graph:collection:latin-uppercase-a-z`
* **Golden Snapshot CIDv1**: `bafyreie5eeznusjiimg7l666feoxwvvk62pbhs6hb7cryh2ana53gm2uqm`

### Decimal Digits Snapshot (Phase 2)
* **Collection URN**: `urn:language-graph:collection:decimal-digits-0-9`
* **Golden Snapshot CIDv1**: `bafyreig3oqpkm3gpsmcs7f25u4vmyvkzczwhjqfvs4iumwpi5uelb353s4`

### Basic Whitespace Snapshot (Phase 2)
* **Collection URN**: `urn:language-graph:collection:basic-english-whitespace`
* **Golden Snapshot CIDv1**: `bafyreidh3pi73vtvgkpt7gbbpbbx2lbi42yracmlnagtfljkfmi4mv36ky`

### Basic Punctuation Snapshot (Phase 2)
* **Collection URN**: `urn:language-graph:collection:basic-english-punctuation`
* **Golden Snapshot CIDv1**: `bafyreidh6nez45kqwkc5ue6c5fvhcblmtehlbae5ubiafmt77rpazrzyuq`

---

## 3. Golden Profile Snapshot CID (Phase 2)

The profile snapshot groups all 5 collections under an ordered references list to form the resolver environment.

* **Profile URN**: `urn:language-graph:profile:basic-english-written-text`
* **Golden Snapshot CIDv1**: `bafyreic5acpnm6zr4cp6jl3xm425kwft77qegml2mhxftrwclkelnqplry`

---

## 4. Golden Revision CIDs (Phase 2.1 Printable ASCII)

These are the deterministic CIDv1 identifiers generated for some key supplemental characters of Phase 2.1:

| Character | Stable Entity URN | Golden Revision CIDv1 | Phase |
| --------- | ----------------- | --------------------- | ----- |
| **#** | `urn:language-graph:grapheme:nfc:0023` | `bafyreiajj25zb3zic6pcu7655fsk4a7mvclwgiof3i44eovx2zxodioo7m` | Phase 2.1 |
| **$** | `urn:language-graph:grapheme:nfc:0024` | `bafyreidh4oihmqyykdz7dwsvy4yndkg7ihw4hf3jcn3h2z46fqr5pldznu` | Phase 2.1 |
| **\\** | `urn:language-graph:grapheme:nfc:005c` | `bafyreigsg5oxzm26o4v35tpamxbbg2xrgc65a32x44zxpawxyqssata6cm` | Phase 2.1 |
| **`** | `urn:language-graph:grapheme:nfc:0060` | `bafyreibvmqwgeanqyomvln2zs2ixjw2jystszzpccukyzcz433nsdmyvke` | Phase 2.1 |
| **~** | `urn:language-graph:grapheme:nfc:007e` | `bafyreicx2xpvp2d4gzijs7q3irpd56ymjooalnyzwvkpypaeblzkoken4e` | Phase 2.1 |

---

## 5. Golden Supplemental Snapshot CID (Phase 2.1)

### ASCII Supplemental Symbols Snapshot
* **Collection URN**: `urn:language-graph:collection:ascii-supplemental-symbols`
* **Golden Snapshot CIDv1**: `bafyreiaczeqz45ypyr53lbmyyar3ppquj2zusctubs4wqmhaqxxxnjl6zm`

---

## 6. Golden Profile Snapshot CID (Phase 2.1)

The profile snapshot groups all 6 collections under an ordered references list to form the resolver environment.

* **Profile URN**: `urn:language-graph:profile:printable-ascii-text`
* **Golden Snapshot CIDv1**: `bafyreidfdj3hw7gv5rt7bpsfkrkhuptprcjlwzpaq3yektnztec4caqdn4`

