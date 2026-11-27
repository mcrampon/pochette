# Los textos que comparten las dos aplicaciones: duraciones, recuentos, fechas.

## Cómo se escriben los números

number-group-separator = .
number-decimal-separator = ,

## Duraciones

duration-minutes = { $minutes } min
duration-hours = { $hours } h
duration-hours-minutes = { $hours } h { $minutes } min

## Recuentos

track-count =
    { $count ->
        [one] { $count } pista
       *[other] { $count } pistas
    }
play-count =
    { $count ->
        [one] { $count } reproducción
       *[other] { $count } reproducciones
    }

## Un lanzamiento

audio-quality = { $bits } bits / { $rate } kHz
release-type-album = Álbum
release-type-single = Single
release-type-ep = EP
release-type-live = En directo
release-type-compilation = Recopilatorio
release-type-box-set = Caja recopilatoria
release-date = { $day } { $month } { $year }

## Cuándo pasó algo

ago-today = hoy
ago-yesterday = ayer
ago-days =
    { $days ->
        [one] hace { $days } día
       *[other] hace { $days } días
    }
ago-week = hace una semana
ago-weeks =
    { $weeks ->
        [one] hace { $weeks } semana
       *[other] hace { $weeks } semanas
    }
ago-month = en { $month }
ago-month-year = en { $month } de { $year }
since-now = ahora mismo
since-minutes =
    { $minutes ->
        [one] hace un minuto
       *[other] hace { $minutes } minutos
    }
since-hours =
    { $hours ->
        [one] hace una hora
       *[other] hace { $hours } horas
    }

## Meses

month-1 = enero
month-2 = febrero
month-3 = marzo
month-4 = abril
month-5 = mayo
month-6 = junio
month-7 = julio
month-8 = agosto
month-9 = septiembre
month-10 = octubre
month-11 = noviembre
month-12 = diciembre
month-short-1 = ene
month-short-2 = feb
month-short-3 = mar
month-short-4 = abr
month-short-5 = may
month-short-6 = jun
month-short-7 = jul
month-short-8 = ago
month-short-9 = sept
month-short-10 = oct
month-short-11 = nov
month-short-12 = dic

## Ordenar los álbumes

sort-date-added = Fecha de adición
sort-title = Título
sort-artist = Artista
sort-release-date = Fecha de lanzamiento
sort-last-played = Última reproducción
sort-play-count = Reproducciones
order-newest-first = Más recientes primero
order-oldest-first = Más antiguos primero
order-a-to-z = De la A a la Z
order-z-to-a = De la Z a la A
order-most-recent-first = Más recientes primero
order-least-recent-first = Menos recientes primero
order-most-played-first = Más escuchados primero
order-least-played-first = Menos escuchados primero
