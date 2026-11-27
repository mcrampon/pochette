# Teksty wspólne obu aplikacji, po polsku: długości, liczby, daty.

## Zapis liczb

# Spacja nierozdzielająca (U+00A0) między tysiącami: 1 204 h.
number-group-separator = { " " }
number-decimal-separator = ,

## Długości

duration-minutes = { $minutes } min
duration-hours = { $hours } h
duration-hours-minutes = { $hours } h { $minutes } min

## Liczby

track-count =
    { $count ->
        [one] { $count } utwór
        [few] { $count } utwory
        [many] { $count } utworów
       *[other] { $count } utworu
    }
play-count =
    { $count ->
        [one] { $count } odtworzenie
        [few] { $count } odtworzenia
        [many] { $count } odtworzeń
       *[other] { $count } odtworzenia
    }

## Wydanie

audio-quality =
    { $bits ->
        [one] { $bits } bit
        [few] { $bits } bity
        [many] { $bits } bitów
       *[other] { $bits } bitu
    } / { $rate } kHz
release-type-album = Album
release-type-single = Singiel
release-type-ep = EP
release-type-live = Na żywo
release-type-compilation = Kompilacja
release-type-box-set = Box set
# Miesiąc w dopełniaczu, skrócony: 13 lis 2026.
release-date =
    { $day } { $number ->
        [1] sty
        [2] lut
        [3] mar
        [4] kwi
        [5] maj
        [6] cze
        [7] lip
        [8] sie
        [9] wrz
        [10] paź
        [11] lis
        [12] gru
       *[other] { $month }
    } { $year }

## Kiedy coś się wydarzyło

ago-today = dzisiaj
ago-yesterday = wczoraj
ago-days =
    { $days ->
        [one] { $days } dzień temu
        [few] { $days } dni temu
        [many] { $days } dni temu
       *[other] { $days } dnia temu
    }
ago-week = tydzień temu
ago-weeks =
    { $weeks ->
        [one] { $weeks } tydzień temu
        [few] { $weeks } tygodnie temu
        [many] { $weeks } tygodni temu
       *[other] { $weeks } tygodnia temu
    }
# Miejscownik: „w marcu”.
ago-month =
    { $number ->
        [1] w styczniu
        [2] w lutym
        [3] w marcu
        [4] w kwietniu
        [5] w maju
        [6] w czerwcu
        [7] w lipcu
        [8] w sierpniu
        [9] we wrześniu
        [10] w październiku
        [11] w listopadzie
        [12] w grudniu
       *[other] w { $month }
    }
ago-month-year =
    { $number ->
        [1] w styczniu
        [2] w lutym
        [3] w marcu
        [4] w kwietniu
        [5] w maju
        [6] w czerwcu
        [7] w lipcu
        [8] w sierpniu
        [9] we wrześniu
        [10] w październiku
        [11] w listopadzie
        [12] w grudniu
       *[other] w { $month }
    } { $year }
since-now = przed chwilą
since-minutes =
    { $minutes ->
        [one] minutę temu
        [few] { $minutes } minuty temu
        [many] { $minutes } minut temu
       *[other] { $minutes } minuty temu
    }
since-hours =
    { $hours ->
        [one] godzinę temu
        [few] { $hours } godziny temu
        [many] { $hours } godzin temu
       *[other] { $hours } godziny temu
    }

## Miesiące

month-1 = Styczeń
month-2 = Luty
month-3 = Marzec
month-4 = Kwiecień
month-5 = Maj
month-6 = Czerwiec
month-7 = Lipiec
month-8 = Sierpień
month-9 = Wrzesień
month-10 = Październik
month-11 = Listopad
month-12 = Grudzień
month-short-1 = sty
month-short-2 = lut
month-short-3 = mar
month-short-4 = kwi
month-short-5 = maj
month-short-6 = cze
month-short-7 = lip
month-short-8 = sie
month-short-9 = wrz
month-short-10 = paź
month-short-11 = lis
month-short-12 = gru

## Sortowanie albumów

sort-date-added = Data dodania
sort-title = Tytuł
sort-artist = Wykonawca
sort-release-date = Data wydania
sort-last-played = Ostatnio odtwarzane
sort-play-count = Liczba odtworzeń
order-newest-first = Od najnowszych
order-oldest-first = Od najstarszych
order-a-to-z = Od A do Z
order-z-to-a = Od Z do A
order-most-recent-first = Od ostatnio odtwarzanych
order-least-recent-first = Od najdawniej odtwarzanych
order-most-played-first = Od najczęściej odtwarzanych
order-least-played-first = Od najrzadziej odtwarzanych
