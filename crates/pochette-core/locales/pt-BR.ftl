# Os textos que os dois apps mostram, como o núcleo os escreve: durações, contagens, datas.

## Como os números são escritos

number-group-separator = .
number-decimal-separator = ,

## Durações

duration-minutes = { $minutes } min
duration-hours = { $hours } h
duration-hours-minutes = { $hours } h { $minutes } min

## Contagens

track-count =
    { $count ->
        [0] { $count } faixas
        [one] { $count } faixa
       *[other] { $count } faixas
    }
play-count =
    { $count ->
        [0] { $count } reproduções
        [one] { $count } reprodução
       *[other] { $count } reproduções
    }

## Um lançamento

audio-quality = { $bits } bits / { $rate } kHz
release-type-album = Álbum
release-type-single = Single
release-type-ep = EP
release-type-live = Ao vivo
release-type-compilation = Coletânea
release-type-box-set = Box
release-date = { $day } de { $month } de { $year }

## Quando algo aconteceu

ago-today = hoje
ago-yesterday = ontem
ago-days =
    { $days ->
        [one] há { $days } dia
       *[other] há { $days } dias
    }
ago-week = há uma semana
ago-weeks =
    { $weeks ->
        [one] há { $weeks } semana
       *[other] há { $weeks } semanas
    }
ago-month = em { $month }
ago-month-year = em { $month } de { $year }
since-now = agora mesmo
since-minutes =
    { $minutes ->
        [one] há um minuto
       *[other] há { $minutes } minutos
    }
since-hours =
    { $hours ->
        [one] há uma hora
       *[other] há { $hours } horas
    }

## Meses

month-1 = janeiro
month-2 = fevereiro
month-3 = março
month-4 = abril
month-5 = maio
month-6 = junho
month-7 = julho
month-8 = agosto
month-9 = setembro
month-10 = outubro
month-11 = novembro
month-12 = dezembro
month-short-1 = jan.
month-short-2 = fev.
month-short-3 = mar.
month-short-4 = abr.
month-short-5 = mai.
month-short-6 = jun.
month-short-7 = jul.
month-short-8 = ago.
month-short-9 = set.
month-short-10 = out.
month-short-11 = nov.
month-short-12 = dez.

## Ordenação dos álbuns

sort-date-added = Data de adição
sort-title = Título
sort-artist = Artista
sort-release-date = Data de lançamento
sort-last-played = Última reprodução
sort-play-count = Número de reproduções
order-newest-first = Mais recentes primeiro
order-oldest-first = Mais antigos primeiro
order-a-to-z = De A a Z
order-z-to-a = De Z a A
order-most-recent-first = Mais recentes primeiro
order-least-recent-first = Menos recentes primeiro
order-most-played-first = Mais tocados primeiro
order-least-played-first = Menos tocados primeiro
