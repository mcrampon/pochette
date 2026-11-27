# Die Texte beider Apps, wie der Kern sie formuliert: Längen, Anzahlen, Daten.

## Zahlen

number-group-separator = .
number-decimal-separator = ,

## Längen

duration-minutes = { $minutes } Min.
duration-hours = { $hours } Std.
duration-hours-minutes = { $hours } Std. { $minutes } Min.

## Anzahlen

track-count =
    { $count ->
        [one] { $count } Titel
       *[other] { $count } Titel
    }
play-count =
    { $count ->
        [one] { $count } Wiedergabe
       *[other] { $count } Wiedergaben
    }

## Eine Veröffentlichung

audio-quality = { $bits } Bit / { $rate } kHz
release-type-album = Album
release-type-single = Single
release-type-ep = EP
release-type-live = Live
release-type-compilation = Kompilation
release-type-box-set = Box-Set
release-date = { $day }. { $month } { $year }

## Wann etwas war

ago-today = heute
ago-yesterday = gestern
ago-days =
    { $days ->
        [one] vor { $days } Tag
       *[other] vor { $days } Tagen
    }
ago-week = vor einer Woche
ago-weeks =
    { $weeks ->
        [one] vor { $weeks } Woche
       *[other] vor { $weeks } Wochen
    }
ago-month = im { $month }
ago-month-year = im { $month } { $year }
since-now = gerade eben
since-minutes =
    { $minutes ->
        [one] vor einer Minute
       *[other] vor { $minutes } Minuten
    }
since-hours =
    { $hours ->
        [one] vor einer Stunde
       *[other] vor { $hours } Stunden
    }

## Monate

month-1 = Januar
month-2 = Februar
month-3 = März
month-4 = April
month-5 = Mai
month-6 = Juni
month-7 = Juli
month-8 = August
month-9 = September
month-10 = Oktober
month-11 = November
month-12 = Dezember
month-short-1 = Jan.
month-short-2 = Feb.
month-short-3 = März
month-short-4 = Apr.
month-short-5 = Mai
month-short-6 = Juni
month-short-7 = Juli
month-short-8 = Aug.
month-short-9 = Sept.
month-short-10 = Okt.
month-short-11 = Nov.
month-short-12 = Dez.

## Sortierung der Alben

sort-date-added = Hinzufügedatum
sort-title = Titel
sort-artist = Künstler
sort-release-date = Erscheinungsdatum
sort-last-played = Zuletzt gehört
sort-play-count = Wiedergaben
# Nach der Sortierung: „Hinzufügedatum, Neueste zuerst“.
order-newest-first = Neueste zuerst
order-oldest-first = Älteste zuerst
order-a-to-z = A bis Z
order-z-to-a = Z bis A
order-most-recent-first = Kürzlich gehörte zuerst
order-least-recent-first = Lange nicht gehörte zuerst
order-most-played-first = Meistgehörte zuerst
order-least-played-first = Am wenigsten gehörte zuerst
