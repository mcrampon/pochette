# The texts both apps show, as the core words them, in British English: lengths, counts, dates.
# A number's $count, $hours… picks the plural form: { $count -> [one] … *[other] … }.

## How numbers are written

# Between thousands, as in 1,204 h.
number-group-separator = ,
# Before the decimals, as in 44.1 kHz.
number-decimal-separator = .

## Lengths

# An album's, or time spent listening, under an hour.
duration-minutes = { $minutes } min
# An album's, on the hour, or time spent listening, past an hour. $hours may have a separator.
duration-hours = { $hours } h
# An album's, past an hour.
duration-hours-minutes = { $hours } h { $minutes } min

## Counts

track-count =
    { $count ->
        [one] { $count } track
       *[other] { $count } tracks
    }
play-count =
    { $count ->
        [one] { $count } play
       *[other] { $count } plays
    }

## A release

# The best quality it comes in. $rate may have decimals, as 44.1.
audio-quality = { $bits }-bit / { $rate } kHz
release-type-album = Album
release-type-single = Single
release-type-ep = EP
release-type-live = Live
release-type-compilation = Compilation
release-type-box-set = Box Set
# When it came out. $month is the month's short name, $number its number, from 1.
release-date = { $day } { $month } { $year }

## When something happened

ago-today = today
ago-yesterday = yesterday
ago-days = { $days } days ago
ago-week = a week ago
ago-weeks = { $weeks } weeks ago
# Earlier this year. $month is the month's name, $number its number, from 1.
ago-month = in { $month }
# Another year. $month is the month's name, $number its number, from 1.
ago-month-year = in { $month } { $year }
since-now = just now
since-minutes =
    { $minutes ->
        [one] a minute ago
       *[other] { $minutes } minutes ago
    }
since-hours =
    { $hours ->
        [one] an hour ago
       *[other] { $hours } hours ago
    }

## Months

month-1 = January
month-2 = February
month-3 = March
month-4 = April
month-5 = May
month-6 = June
month-7 = July
month-8 = August
month-9 = September
month-10 = October
month-11 = November
month-12 = December
month-short-1 = Jan
month-short-2 = Feb
month-short-3 = Mar
month-short-4 = Apr
month-short-5 = May
month-short-6 = Jun
month-short-7 = Jul
month-short-8 = Aug
month-short-9 = Sep
month-short-10 = Oct
month-short-11 = Nov
month-short-12 = Dec

## Sorting the albums

sort-date-added = Date Added
sort-title = Title
sort-artist = Artist
sort-release-date = Release Date
sort-last-played = Last Played
sort-play-count = Play Count
# How the albums are sorted, after the sort: "Date Added, Newest First".
order-newest-first = Newest First
order-oldest-first = Oldest First
order-a-to-z = A to Z
order-z-to-a = Z to A
order-most-recent-first = Most Recent First
order-least-recent-first = Least Recent First
order-most-played-first = Most Played First
order-least-played-first = Least Played First
