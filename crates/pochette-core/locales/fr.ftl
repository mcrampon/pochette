# Les textes communs aux deux applications, en français (France) : durées, nombres, dates.

## Écriture des nombres

number-group-separator =  
number-decimal-separator = ,

## Durées

duration-minutes = { $minutes } min
duration-hours = { $hours } h
duration-hours-minutes = { $hours } h { $minutes } min

## Nombres

track-count =
    { $count ->
        [one] { $count } titre
       *[other] { $count } titres
    }
play-count =
    { $count ->
        [one] { $count } écoute
       *[other] { $count } écoutes
    }

## Une parution

audio-quality = { $bits } bits / { $rate } kHz
release-type-album = Album
release-type-single = Single
release-type-ep = EP
release-type-live = Live
release-type-compilation = Compilation
release-type-box-set = Coffret
release-date =
    { $day ->
        [1] 1er
       *[other] { $day }
    } { $month } { $year }

## Quand

ago-today = aujourd'hui
ago-yesterday = hier
ago-days =
    { $days ->
        [one] il y a { $days } jour
       *[other] il y a { $days } jours
    }
ago-week = il y a une semaine
ago-weeks =
    { $weeks ->
        [one] il y a { $weeks } semaine
       *[other] il y a { $weeks } semaines
    }
ago-month = en { $month }
ago-month-year = en { $month } { $year }
since-now = à l'instant
since-minutes =
    { $minutes ->
        [one] il y a une minute
       *[other] il y a { $minutes } minutes
    }
since-hours =
    { $hours ->
        [one] il y a une heure
       *[other] il y a { $hours } heures
    }

## Mois

month-1 = janvier
month-2 = février
month-3 = mars
month-4 = avril
month-5 = mai
month-6 = juin
month-7 = juillet
month-8 = août
month-9 = septembre
month-10 = octobre
month-11 = novembre
month-12 = décembre
month-short-1 = janv.
month-short-2 = févr.
month-short-3 = mars
month-short-4 = avr.
month-short-5 = mai
month-short-6 = juin
month-short-7 = juil.
month-short-8 = août
month-short-9 = sept.
month-short-10 = oct.
month-short-11 = nov.
month-short-12 = déc.

## Tri des albums

sort-date-added = Date d'ajout
sort-title = Titre
sort-artist = Artiste
sort-release-date = Date de sortie
sort-last-played = Dernière écoute
sort-play-count = Nombre d'écoutes
order-newest-first = Plus récents d'abord
order-oldest-first = Plus anciens d'abord
order-a-to-z = De A à Z
order-z-to-a = De Z à A
order-most-recent-first = Plus récents d'abord
order-least-recent-first = Plus anciens d'abord
order-most-played-first = Plus écoutés d'abord
order-least-played-first = Moins écoutés d'abord
