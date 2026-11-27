# Pochette 桌面版的界面文字，简体中文。
# 时长、数量和日期在核心的文字里：crates/pochette-core/locales。

## 主窗口

search-qobuz = 搜索 Qobuz
main-menu = 主菜单
now-playing = 正在播放
tab-discover = 发现
tab-albums = 专辑
menu-stats = 听歌统计
menu-sign-out = 退出登录
previews-only = 此账号只能播放 30 秒试听片段。

## 启动与登录

unreachable = 无法连接 Qobuz
try-again = 重试
sign-in-tagline = 你的 Qobuz 音乐库，一张专辑接一张专辑。
sign-in = 使用 Qobuz 登录
sign-in-hint =
    登录时会在浏览器中打开 Qobuz。
    需要订阅 Qobuz。
sign-in-continue = 请在浏览器中继续
sign-in-continue-hint =
    请在刚刚打开的浏览器标签页中登录 Qobuz。
    完成后，Pochette 会立即继续。
sign-in-open-again = 重新打开浏览器
sign-in-copy-link = 复制链接
sign-in-cancel = 取消
session-expired = 登录已过期，请重新登录。
sign-in-cannot-save = 无法保存登录状态：{ $error }
sign-in-timed-out = 登录超时，请重试。

## 设置

settings = 设置
settings-language = 语言
# 跟随系统语言。
settings-language-system = 跟随系统
settings-language-restart = 下次打开 Pochette 时生效。
settings-devices = 设备

## 设置中的设备

devices-about = 已配对的设备在同一网络中时，会互相同步你的播放记录，用于统计和播放次数。
devices-paired = 已配对的设备
devices-in-background = Pochette 关闭时继续同步
devices-pair-a-device = 配对设备
devices-pair-hint = 在另一台设备上也打开 Pochette 的设置，它就会出现在这里。
devices-looking = 正在查找此网络中的设备…
devices-by-address = 通过地址配对
devices-address = 设备地址，例如 192.168.1.20
devices-pair = 配对
devices-firewall =
    如果设备之间互相找不到，可能是网络或防火墙阻挡了连接。Pochette 需要 TCP 端口 { $port } 进行同步，并需要 UDP 端口 5353 来查找设备。
devices-pair-with = 与 { $name } 配对
devices-cancel = 取消
devices-paired-with = 已与 { $name } 配对。
devices-not-paired = 未与 { $name } 配对。
devices-declined = { $name } 拒绝了配对。
devices-pairing-failed = 无法与 { $name } 配对：{ $reason }
devices-syncing = 正在同步…
# $when 表示多久以前，例如“3 分钟前”。
devices-synced = 上次同步：{ $when }
devices-synced-error = 上次同步：{ $when }。{ $error }
devices-not-synced = 尚未同步
devices-forget = 忘记
devices-forget-tooltip = 停止与此设备同步
devices-cannot-change = 无法更改：{ $error }
devices-connecting = 正在连接…
devices-confirm = 请确认 { $name } 显示的是相同的代码，然后在两台设备上都点击配对。
devices-waiting = 正在等待 { $name } 确认…
sync-no-bus = 同步需要 D-Bus 会话总线：{ $error }
sync-cannot-start = 无法开始同步：{ $error }
sync-no-answer = 对方没有响应

## 通用

back = 返回
play = 播放
pause = 暂停
previous = 上一首
next = 下一首
see-all = 查看全部
favorite-add = 添加到我的专辑
favorite-remove = 从我的专辑中移除
genres = 流派
genres-all = 所有流派
decades = 年代
decades-all = 所有年代
# 以首年表示的年代：“1970年代”。
decade = { $decade }年代
# 专辑的排序方式：$sort 如“添加日期”，$order 如“从新到旧”。
sort-by = 排序方式：{ $sort }，{ $order }
sort-picked = { $sort }，{ $order }

## 专辑标签页

albums-cannot-load = 无法加载你的专辑
albums-none = 没有已收藏的专辑
albums-none-hint = 你在 Qobuz 上收藏的专辑会显示在这里。
albums-search = 搜索专辑、艺人、厂牌
albums-random = 随机播放一张专辑
albums-random-filtered = 从这些专辑中随机播放一张
albums-no-match = 没有匹配的专辑
albums-clear-filters = 清除搜索和筛选

## 发现标签页

discover-cannot-load = 无法加载“发现”
discover-section-cannot-load = 无法加载这些专辑
discover-section-empty = 这里没有专辑
discover-new-releases = 新发行
discover-press-awards = 媒体奖项
discover-ideal-discography = 理想唱片库

## 搜索 Qobuz

# 搜索内容，作为页面标题。
search-query = “{ $query }”
search-failed = 无法搜索 Qobuz
search-nothing-found = 未找到任何结果
search-no-albums = 未找到专辑
search-artists = 艺人

## 专辑

album-cannot-load = 无法加载曲目：{ $error }
album-about = 关于这张专辑
album-copy-link = 复制链接
album-link-copied = 已复制链接
# $plays 如“12 次播放”，$last 表示何时，如“昨天”。
album-played = { $plays } · 上次播放：{ $last }
hi-res = Hi-Res
album-disc = 第 { $number } 张碟
track-unavailable = 暂不支持在线播放

## 艺人

# 显示在艺人名字上方。
artist-kicker = 艺人
artist-more = 展开
artist-less = 收起
artist-cannot-load = 无法加载此艺人：{ $error }
artist-no-albums = 此艺人没有可在线播放的专辑。
artist-similar = 相似艺人
releases-albums = 专辑
releases-eps-singles = EP 和单曲
releases-live = 现场
releases-compilations = 合辑
releases-other = 其他发行
releases-cannot-load = 无法加载这些专辑
releases-empty = 这里没有专辑

## 正在播放

now-playing-quality = 音质
now-playing-show-cover = 显示封面
now-playing-show-lyrics = 显示歌词
now-playing-position = 播放进度
now-playing-volume = 音量
now-playing-tracks = 曲目
output = 输出设备
output-default = 系统默认
quality-best = 最佳音质
quality-hint = 在你的订阅允许范围内，每首曲目都以其最佳音质播放，最高不超过此设置。
quality-mp3 = MP3 · 320 kbps
quality-cd = CD · 16 bit / 44.1 kHz
quality-hi-res-96 = Hi-Res · 最高 24 bit / 96 kHz
quality-hi-res-192 = Hi-Res · 最高 24 bit / 192 kHz
# 正在播放的曲目为 MP3 时的音质。
quality-mp3-playing = MP3 · 320 kbps
playback-cannot-start = 无法开始播放：{ $error }
playback-cannot-stream = 无法播放“{ $title }”：{ $error }
lyrics-failed = 无法加载歌词
lyrics-none = 此曲目没有歌词
lyrics-instrumental = 纯音乐

## 听歌统计

stats-year = 年
stats-month = 月
stats-all-time = 全部时间
stats-whole-year = 全年
stats-top-albums = 最常听的专辑
stats-top-artists = 最常听的艺人
stats-top-genres = 最常听的流派
stats-nothing = 还没有播放记录
stats-nothing-hint = 播放一些专辑吧：你的收听时长、最常听的专辑、艺人和流派都会显示在这里。
stats-listened = 收听时长
stats-album-plays = 专辑播放次数
stats-artists = 艺人
stats-by-year = 按年
stats-by-month = 按月
stats-by-day = 按日
# 某月的某一天：$month 为月份名称（如“3月”），$day 为日期。
stats-day = { $month }{ $day }日
# 柱状图的提示：“3月：12 小时”。
stats-bar = { $name }：{ $time }
# 某流派占收听时长的比例。
stats-share = { $percent }%
