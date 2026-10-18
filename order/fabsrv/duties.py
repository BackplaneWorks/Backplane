"""Import duties and taxes on goods made in China, by destination.

Charged only when the parcel goes duty paid (DDP): the fab collects them with the
freight, so the person pays nothing at the door. Otherwise the courier collects
whatever is due on delivery and the quote says so instead.

US: a flat 35% of the goods on circuit boards and assemblies (the fab's duty-paid
rate since March 2026; tariffs on China change often, check it monthly).
EU: VAT on goods and freight, plus the EUR 3 per-line duty in force from 1 July
2026 (until July 2028). UK, Norway, Switzerland, Australia, Canada, ...: VAT/GST.
"""
EU_VAT = {"AT": 20, "BE": 21, "BG": 20, "HR": 25, "CY": 19, "CZ": 21, "DK": 25, "EE": 24, "FI": 25.5, "FR": 20,
          "DE": 19, "GR": 24, "HU": 27, "IE": 23, "IT": 22, "LV": 21, "LT": 21, "LU": 17, "MT": 18, "NL": 21,
          "PL": 23, "PT": 23, "RO": 21, "SK": 23, "SI": 22, "ES": 21, "SE": 25}
VAT = {"GB": 20, "NO": 25, "CH": 8.1, "AU": 10, "NZ": 15, "CA": 13, "JP": 10, "KR": 10, "SG": 9, "IL": 18,
       "TR": 20, "IS": 24, "IN": 28, "MX": 16, "TW": 5, "HK": 0, "AE": 5, "ZA": 15, "BR": 60}
US_RATE = 35
EUR_LINE = 3.0 * 1.08

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
    if country == "US":
        return f"US import duty, {US_RATE}% of the goods, paid in advance"
    if country in EU_VAT:
        return f"{EU_VAT[country]:g}% VAT and EU import duty, paid in advance"
    if country in VAT:
        return f"{VAT[country]:g}% VAT/GST, paid in advance"
    return "Duties and taxes, if any, are collected on delivery"
