# 两个应用共用的文字，简体中文：时长、数量、日期。
# 中文没有复数变化，所以不需要按数量选择。

## 数字的写法

# 千位分隔符，如 1,204 小时。
number-group-separator = ,
# 小数点，如 44.1 kHz。
number-decimal-separator = .

## 时长

# 专辑时长或收听时长，不足一小时。
duration-minutes = { $minutes } 分钟
# 专辑时长（整小时）或收听时长（超过一小时）。$hours 可能带有分隔符。
duration-hours = { $hours } 小时
# 专辑时长，超过一小时。
duration-hours-minutes = { $hours } 小时 { $minutes } 分钟

## 数量

track-count = { $count } 首曲目
play-count = { $count } 次播放

## 发行

# 最高音质。$rate 可能带小数，如 44.1。
audio-quality = { $bits } bit / { $rate } kHz
release-type-album = 专辑
release-type-single = 单曲
release-type-ep = EP
release-type-live = 现场
release-type-compilation = 合辑
release-type-box-set = 套装
# 发行日期。$month 为月份简称，如“11月”。
release-date = { $year }年{ $month }{ $day }日

## 发生的时间

ago-today = 今天
ago-yesterday = 昨天
ago-days = { $days } 天前
ago-week = 1 周前
ago-weeks = { $weeks } 周前
# 今年早些时候。$month 为月份名称，如“3月”。
ago-month = { $month }
# 其他年份。$month 为月份名称，如“3月”。
ago-month-year = { $year }年{ $month }
since-now = 刚刚
since-minutes = { $minutes } 分钟前
since-hours = { $hours } 小时前

## 月份

month-1 = 1月
month-2 = 2月
month-3 = 3月
month-4 = 4月
month-5 = 5月
month-6 = 6月
month-7 = 7月
month-8 = 8月
month-9 = 9月
month-10 = 10月
month-11 = 11月
month-12 = 12月
month-short-1 = 1月
month-short-2 = 2月
month-short-3 = 3月
month-short-4 = 4月
month-short-5 = 5月
month-short-6 = 6月
month-short-7 = 7月
month-short-8 = 8月
month-short-9 = 9月
month-short-10 = 10月
month-short-11 = 11月
month-short-12 = 12月

## 专辑排序

sort-date-added = 添加日期
sort-title = 标题
sort-artist = 艺人
sort-release-date = 发行日期
sort-last-played = 上次播放
sort-play-count = 播放次数
# 排序方向，跟在排序方式后面：“添加日期，从新到旧”。
order-newest-first = 从新到旧
order-oldest-first = 从旧到新
order-a-to-z = 从 A 到 Z
order-z-to-a = 从 Z 到 A
order-most-recent-first = 从近到远
order-least-recent-first = 从远到近
order-most-played-first = 从多到少
order-least-played-first = 从少到多
