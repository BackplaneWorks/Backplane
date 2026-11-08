"""Import duties and taxes on goods made in China, by destination.

Charged only when the parcel goes duty paid (DDP): the fab collects them with the
freight, so the person pays nothing at the door. Otherwise the courier collects
whatever is due on delivery and the quote says so instead.

US: a flat 35% of the goods on circuit boards and assemblies (the fab's duty-paid
rate since March 2026; tariffs on China change often, check it monthly).
EU: VAT on goods and freight, plus the EUR 3 per-line duty in force from 1 July
2026 (until July 2028). UK, Norway, Switzerland, Australia, Canada, ...: VAT/GST.

A duty-paid parcel also carries the courier's duty and tax disbursement fee (it pays
the duties for us and bills them on): USD 4.44 a parcel to the US and EUR 7.40 to
Greece on the fab's checkouts (2026-11), in the Duties & taxes line.

US sales tax is a line of its own: the fab charges it on every US order, duty paid or
not, on the goods and the shipping (not the duties), at the ship-to's state and local
rate. The quote uses the state's average combined rate; without a state, a rate at the
high end of US combined rates, so a quote does not come in under what is charged.
"""
EU_VAT = {"AT": 20, "BE": 21, "BG": 20, "HR": 25, "CY": 19, "CZ": 21, "DK": 25, "EE": 24, "FI": 25.5, "FR": 20,
          "DE": 19, "GR": 24, "HU": 27, "IE": 23, "IT": 22, "LV": 21, "LT": 21, "LU": 17, "MT": 18, "NL": 21,
          "PL": 23, "PT": 23, "RO": 21, "SK": 23, "SI": 22, "ES": 21, "SE": 25}
VAT = {"GB": 20, "NO": 25, "CH": 8.1, "AU": 10, "NZ": 15, "CA": 13, "JP": 10, "KR": 10, "SG": 9, "IL": 18,
       "TR": 20, "IS": 24, "IN": 28, "MX": 16, "TW": 5, "HK": 0, "AE": 5, "ZA": 15, "BR": 60}
US_RATE = 35
EUR_LINE = 3.0 * 1.08
# the fixed, conservative EUR rate for fees billed in euros (ECB was about 1.16 in 2026-11)
EUR_FEES = 1.20
# the courier's disbursement fee per duty-paid parcel, in USD: seen on the fab's
# checkouts to the US (2026-11-08, two orders) and Greece (EUR 7.40, 2026-11-02);
# elsewhere the EU figure, which is the higher
DISBURSEMENT = {"US": 4.44}
DISBURSEMENT_OTHER = round(7.40 * EUR_FEES, 2)

# US combined state and average local sales tax rates, percent, as of 1 July 2026:
# Tax Foundation, "State and Local Sales Tax Rates, Midyear 2026"
# (https://taxfoundation.org/data/all/state/2026-sales-tax-rates-midyear/). A city's
# rate can be higher than its state's average (Seattle 10.55% against 9.57%); the
# fee covers that much. Alaska has no state tax but its towns tax remote sales.
US_SALES_TAX = {"AL": 9.46, "AK": 1.82, "AZ": 8.54, "AR": 9.48, "CA": 9.03, "CO": 7.89, "CT": 6.35, "DE": 0.0,
                "DC": 6.00, "FL": 6.98, "GA": 7.56, "HI": 4.50, "ID": 6.03, "IL": 8.98, "IN": 7.00, "IA": 6.94,
                "KS": 8.71, "KY": 6.00, "LA": 10.13, "ME": 5.50, "MD": 6.00, "MA": 6.25, "MI": 6.00, "MN": 8.14,
                "MS": 7.06, "MO": 8.44, "MT": 0.0, "NE": 6.98, "NV": 8.24, "NH": 0.0, "NJ": 6.60, "NM": 7.68,
                "NY": 8.54, "NC": 7.10, "ND": 7.09, "OH": 7.29, "OK": 9.06, "OR": 0.0, "PA": 6.34, "RI": 7.00,
                "SC": 7.49, "SD": 6.11, "TN": 9.61, "TX": 8.20, "UT": 7.42, "VT": 6.43, "VA": 5.77, "WA": 9.57,
                "WV": 6.60, "WI": 5.72, "WY": 5.39}
# without a state: about the highest combined rates (Louisiana's average, the big
# cities of Washington, Illinois, Alabama and Tennessee)
US_SALES_TAX_NO_STATE = 10.25
STATES = {"AL": "Alabama", "AK": "Alaska", "AZ": "Arizona", "AR": "Arkansas", "CA": "California", "CO": "Colorado",
          "CT": "Connecticut", "DE": "Delaware", "DC": "the District of Columbia", "FL": "Florida", "GA": "Georgia",
          "HI": "Hawaii", "ID": "Idaho", "IL": "Illinois", "IN": "Indiana", "IA": "Iowa", "KS": "Kansas",
          "KY": "Kentucky", "LA": "Louisiana", "ME": "Maine", "MD": "Maryland", "MA": "Massachusetts", "MI": "Michigan",
          "MN": "Minnesota", "MS": "Mississippi", "MO": "Missouri", "MT": "Montana", "NE": "Nebraska", "NV": "Nevada",
          "NH": "New Hampshire", "NJ": "New Jersey", "NM": "New Mexico", "NY": "New York", "NC": "North Carolina",
          "ND": "North Dakota", "OH": "Ohio", "OK": "Oklahoma", "OR": "Oregon", "PA": "Pennsylvania",
          "RI": "Rhode Island", "SC": "South Carolina", "SD": "South Dakota", "TN": "Tennessee", "TX": "Texas",
          "UT": "Utah", "VT": "Vermont", "VA": "Virginia", "WA": "Washington", "WV": "West Virginia",
          "WI": "Wisconsin", "WY": "Wyoming"}

NAMES = {"US": "the United States", "GB": "the United Kingdom", "DE": "Germany", "FR": "France", "IT": "Italy",
         "ES": "Spain", "NL": "the Netherlands", "BE": "Belgium", "AT": "Austria", "GR": "Greece", "PT": "Portugal",
         "IE": "Ireland", "SE": "Sweden", "DK": "Denmark", "FI": "Finland", "PL": "Poland", "CZ": "Czechia",
         "CA": "Canada", "AU": "Australia", "NZ": "New Zealand", "JP": "Japan", "KR": "South Korea", "SG": "Singapore",
         "CH": "Switzerland", "NO": "Norway", "IL": "Israel", "IN": "India", "MX": "Mexico", "BR": "Brazil",
         "TW": "Taiwan", "HK": "Hong Kong", "CY": "Cyprus", "RO": "Romania", "BG": "Bulgaria", "HU": "Hungary",
         "SK": "Slovakia", "SI": "Slovenia", "HR": "Croatia", "LT": "Lithuania", "LV": "Latvia", "EE": "Estonia",
         "LU": "Luxembourg", "MT": "Malta", "TR": "Turkey", "ZA": "South Africa", "AE": "the United Arab Emirates",
         "IS": "Iceland"}


def name(country):
    return NAMES.get(country, country)


def estimate(country, goods, freight, lines, ddp):
    """What goes in the quote for duties and taxes (0 when the courier collects them instead)."""
    if not ddp:
        return 0.0
    if country == "US":
        return round(goods * US_RATE / 100, 2)
    if country in EU_VAT:
        return round((goods + freight) * EU_VAT[country] / 100 + EUR_LINE * max(1, lines), 2)
    if country in VAT:
        return round((goods + freight) * VAT[country] / 100, 2)
    return 0.0


def note(country, ddp):
    if not ddp:
        return "Collected by the courier on delivery, if any is due"
    fee = " and the courier's fee for paying them"
    if country == "US":
        return f"US import duty ({US_RATE}% of the goods){fee}, paid in advance"
    if country in EU_VAT:
        return f"{EU_VAT[country]:g}% VAT and EU import duty{fee}, paid in advance"
    if country in VAT:
        return f"{VAT[country]:g}% VAT/GST{fee}, paid in advance"
    return "Duties and taxes, if any, are collected on delivery"


def disbursement(country, parcels, ddp):
    """The courier's fee for paying the duties of each duty-paid parcel (0 when the courier collects them instead)."""
    if not ddp or not parcels or not (country == "US" or country in EU_VAT or country in VAT):
        return 0.0
    return round(DISBURSEMENT.get(country, DISBURSEMENT_OTHER) * parcels, 2)


def sales_tax_rate(country, region):
    """(percent, known): the US sales tax rate for a state, or the no-state rate; None outside the US."""
    if country != "US":
        return None
    if region in US_SALES_TAX:
        return US_SALES_TAX[region], True
    return US_SALES_TAX_NO_STATE, False


def sales_tax(country, region, goods, freight):
    """{amount, rate, region, base, known} for a US quote (None elsewhere): on the goods and the shipping."""
    r = sales_tax_rate(country, region)
    if r is None:
        return None
    rate, known = r
    base = round(goods + freight, 2)
    return {"amount": round(base * rate / 100, 2), "rate": rate, "region": region if known else "", "base": base, "known": known}


def sales_tax_note(t):
    if not t["known"]:
        return (f"Estimated at {t['rate']:g}% without the state it ships to; "
                "any difference is refunded or asked for")
    if not t["rate"]:
        return f"{STATES[t['region']]} has no sales tax"
    return f"{STATES[t['region']]}, {t['rate']:g}% average state and local rate, on the goods and shipping"
