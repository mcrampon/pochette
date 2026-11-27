# I testi che entrambe le app mostrano, come li scrive il core: durate, conteggi, date.

## Come si scrivono i numeri

number-group-separator = .
number-decimal-separator = ,

## Durate

duration-minutes = { $minutes } min
duration-hours = { $hours } h
duration-hours-minutes = { $hours } h { $minutes } min

## Conteggi

track-count =
    { $count ->
        [one] { $count } brano
       *[other] { $count } brani
    }
play-count =
    { $count ->
        [one] { $count } ascolto
       *[other] { $count } ascolti
    }

## Un'uscita

audio-quality = { $bits } bit / { $rate } kHz
release-type-album = Album
release-type-single = Singolo
release-type-ep = EP
release-type-live = Live
release-type-compilation = Compilation
release-type-box-set = Cofanetto
release-date = { $day } { $month } { $year }

## Quando è successo

ago-today = oggi
ago-yesterday = ieri
ago-days =
    { $days ->
        [one] { $days } giorno fa
       *[other] { $days } giorni fa
    }
ago-week = una settimana fa
ago-weeks =
    { $weeks ->
        [one] { $weeks } settimana fa
       *[other] { $weeks } settimane fa
    }
# "ad aprile", "ad agosto", "a marzo".
ago-month =
    { $number ->
        [4] ad { $month }
        [8] ad { $month }
       *[other] a { $month }
    }
ago-month-year =
    { $number ->
        [4] ad { $month } { $year }
        [8] ad { $month } { $year }
       *[other] a { $month } { $year }
    }
since-now = proprio ora
since-minutes =
    { $minutes ->
        [one] un minuto fa
       *[other] { $minutes } minuti fa
    }
since-hours =
    { $hours ->
        [one] un'ora fa
       *[other] { $hours } ore fa
    }

## Mesi

month-1 = gennaio
month-2 = febbraio
month-3 = marzo
month-4 = aprile
month-5 = maggio
month-6 = giugno
month-7 = luglio
month-8 = agosto
month-9 = settembre
month-10 = ottobre
month-11 = novembre
month-12 = dicembre
month-short-1 = gen
month-short-2 = feb
month-short-3 = mar
month-short-4 = apr
month-short-5 = mag
month-short-6 = giu
month-short-7 = lug
month-short-8 = ago
month-short-9 = set
month-short-10 = ott
month-short-11 = nov
month-short-12 = dic

## Ordinare gli album

sort-date-added = Data di aggiunta
sort-title = Titolo
sort-artist = Artista
sort-release-date = Data di uscita
sort-last-played = Ultimo ascolto
sort-play-count = Numero di ascolti
order-newest-first = Prima i più recenti
order-oldest-first = Prima i meno recenti
order-a-to-z = Dalla A alla Z
order-z-to-a = Dalla Z alla A
order-most-recent-first = Prima i più recenti
order-least-recent-first = Prima i meno recenti
order-most-played-first = Prima i più ascoltati
order-least-played-first = Prima i meno ascoltati
