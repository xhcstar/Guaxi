// 省 → 地级行政区 静态表（高德 district 接口导出，2026-11 快照）。
// 行政区划多年才调整一次，内嵌后省级目的地不用联网就能先给出保底城市。
// 每个省的 cities 已按离省中心（省会）由近到远排序，省会排在最前。
export const PROVINCE_CITIES = [
  {
    name: "河南省",
    adcode: "410000",
    lng: 113.75309,
    lat: 34.76705,
    cities: [
      { name: "郑州市", adcode: "410100", lng: 113.62535, lat: 34.7463 }, { name: "开封市", adcode: "410200", lng: 114.31428, lat: 34.79808 }, { name: "新乡市", adcode: "410700", lng: 113.92679, lat: 35.30359 }, { name: "焦作市", adcode: "410800", lng: 113.2419, lat: 35.21573 }, { name: "许昌市", adcode: "411000", lng: 113.852, lat: 34.03732 },
      { name: "济源市", adcode: "419001", lng: 112.60235, lat: 35.06906 }, { name: "洛阳市", adcode: "410300", lng: 112.4539, lat: 34.6197 }, { name: "鹤壁市", adcode: "410600", lng: 114.2973, lat: 35.74833 }, { name: "平顶山市", adcode: "410400", lng: 113.19259, lat: 33.76655 }, { name: "漯河市", adcode: "411100", lng: 114.0166, lat: 33.58038 },
      { name: "周口市", adcode: "411600", lng: 114.70122, lat: 33.63465 }, { name: "安阳市", adcode: "410500", lng: 114.39248, lat: 36.09878 }, { name: "濮阳市", adcode: "410900", lng: 115.02925, lat: 35.76273 }, { name: "商丘市", adcode: "411400", lng: 115.65636, lat: 34.41517 }, { name: "驻马店市", adcode: "411700", lng: 114.02199, lat: 33.01404 },
      { name: "南阳市", adcode: "411300", lng: 112.58475, lat: 33.0161 }, { name: "三门峡市", adcode: "411200", lng: 111.20048, lat: 34.7732 }, { name: "信阳市", adcode: "411500", lng: 114.09106, lat: 32.14862 },
    ],
  },
  {
    name: "广东省",
    adcode: "440000",
    lng: 113.26689,
    lat: 23.13331,
    cities: [
      { name: "广州市", adcode: "440100", lng: 113.2645, lat: 23.13006 }, { name: "佛山市", adcode: "440600", lng: 113.12159, lat: 23.02135 }, { name: "东莞市", adcode: "441900", lng: 113.75188, lat: 23.02102 }, { name: "江门市", adcode: "440700", lng: 113.08155, lat: 22.57895 }, { name: "清远市", adcode: "441800", lng: 113.0561, lat: 23.68206 },
      { name: "中山市", adcode: "442000", lng: 113.39252, lat: 22.51702 }, { name: "肇庆市", adcode: "441200", lng: 112.46524, lat: 23.04775 }, { name: "珠海市", adcode: "440400", lng: 113.57689, lat: 22.27164 }, { name: "深圳市", adcode: "440300", lng: 114.05794, lat: 22.54353 }, { name: "惠州市", adcode: "441300", lng: 114.41559, lat: 23.11237 },
      { name: "云浮市", adcode: "445300", lng: 112.04452, lat: 22.91516 }, { name: "河源市", adcode: "441600", lng: 114.70022, lat: 23.74428 }, { name: "韶关市", adcode: "440200", lng: 113.59732, lat: 24.81098 }, { name: "阳江市", adcode: "441700", lng: 111.98343, lat: 21.85685 }, { name: "汕尾市", adcode: "441500", lng: 115.37556, lat: 22.7872 },
      { name: "茂名市", adcode: "440900", lng: 110.92553, lat: 21.66273 }, { name: "梅州市", adcode: "441400", lng: 116.12205, lat: 24.28883 }, { name: "揭阳市", adcode: "445200", lng: 116.37273, lat: 23.55097 }, { name: "潮州市", adcode: "445100", lng: 116.6219, lat: 23.65766 }, { name: "汕头市", adcode: "440500", lng: 116.68196, lat: 23.35415 },
      { name: "湛江市", adcode: "440800", lng: 110.35754, lat: 21.27011 },
    ],
  },
  {
    name: "内蒙古自治区",
    adcode: "150000",
    lng: 111.76523,
    lat: 40.81823,
    cities: [
      { name: "呼和浩特市", adcode: "150100", lng: 111.74881, lat: 40.84213 }, { name: "乌兰察布市", adcode: "150900", lng: 113.13223, lat: 40.99453 }, { name: "包头市", adcode: "150200", lng: 109.95315, lat: 40.62133 }, { name: "鄂尔多斯市", adcode: "150600", lng: 109.78247, lat: 39.60874 }, { name: "巴彦淖尔市", adcode: "150800", lng: 107.38777, lat: 40.74299 },
      { name: "乌海市", adcode: "150300", lng: 106.79415, lat: 39.65505 }, { name: "锡林郭勒盟", adcode: "152500", lng: 116.04739, lat: 43.93321 }, { name: "阿拉善盟", adcode: "152900", lng: 105.72913, lat: 38.85155 }, { name: "赤峰市", adcode: "150400", lng: 118.88761, lat: 42.25688 }, { name: "通辽市", adcode: "150500", lng: 122.24331, lat: 43.65357 },
      { name: "兴安盟", adcode: "152200", lng: 122.0378, lat: 46.08237 }, { name: "呼伦贝尔市", adcode: "150700", lng: 119.77845, lat: 49.16654 },
    ],
  },
  {
    name: "黑龙江省",
    adcode: "230000",
    lng: 126.662,
    lat: 45.74225,
    cities: [
      { name: "哈尔滨市", adcode: "230100", lng: 126.53505, lat: 45.80298 }, { name: "绥化市", adcode: "231200", lng: 126.96871, lat: 46.65415 }, { name: "大庆市", adcode: "230600", lng: 125.10408, lat: 46.5895 }, { name: "牡丹江市", adcode: "231000", lng: 129.63293, lat: 44.55149 }, { name: "伊春市", adcode: "230700", lng: 128.84086, lat: 47.72833 },
      { name: "齐齐哈尔市", adcode: "230200", lng: 123.91819, lat: 47.35489 }, { name: "佳木斯市", adcode: "230800", lng: 130.31892, lat: 46.8 }, { name: "鹤岗市", adcode: "230400", lng: 130.29769, lat: 47.35066 }, { name: "七台河市", adcode: "230900", lng: 131.00302, lat: 45.77118 }, { name: "鸡西市", adcode: "230300", lng: 130.96938, lat: 45.29509 },
      { name: "双鸭山市", adcode: "230500", lng: 131.14156, lat: 46.67616 }, { name: "黑河市", adcode: "231100", lng: 127.52823, lat: 50.24489 }, { name: "大兴安岭地区", adcode: "232700", lng: 124.11786, lat: 50.41129 },
    ],
  },
  {
    name: "新疆维吾尔自治区",
    adcode: "650000",
    lng: 87.62858,
    lat: 43.7933,
    cities: [
      { name: "乌鲁木齐市", adcode: "650100", lng: 87.61682, lat: 43.82538 }, { name: "昌吉回族自治州", adcode: "652300", lng: 87.30899, lat: 44.01104 }, { name: "五家渠市", adcode: "659004", lng: 87.54285, lat: 44.16649 }, { name: "石河子市", adcode: "659001", lng: 86.0804, lat: 44.30537 }, { name: "吐鲁番市", adcode: "650400", lng: 89.25026, lat: 42.97274 },
      { name: "胡杨河市", adcode: "659010", lng: 84.82759, lat: 44.69289 }, { name: "巴音郭楞蒙古自治州", adcode: "652800", lng: 86.1453, lat: 41.76411 }, { name: "铁门关市", adcode: "659006", lng: 85.67029, lat: 41.863 }, { name: "克拉玛依市", adcode: "650200", lng: 84.88924, lat: 45.57771 }, { name: "北屯市", adcode: "659005", lng: 87.83442, lat: 47.32673 },
      { name: "双河市", adcode: "659007", lng: 82.36895, lat: 44.84686 }, { name: "阿勒泰地区", adcode: "654300", lng: 88.14125, lat: 47.84492 }, { name: "博尔塔拉蒙古自治州", adcode: "652700", lng: 82.06636, lat: 44.90604 }, { name: "哈密市", adcode: "650500", lng: 93.51505, lat: 42.81935 }, { name: "塔城地区", adcode: "654200", lng: 82.98032, lat: 46.74536 },
      { name: "白杨市", adcode: "659012", lng: 82.89694, lat: 46.72796 }, { name: "新星市", adcode: "659011", lng: 93.74831, lat: 42.79704 }, { name: "伊犁哈萨克自治州", adcode: "654000", lng: 81.32369, lat: 43.91711 }, { name: "可克达拉市", adcode: "659008", lng: 80.99415, lat: 43.94038 }, { name: "阿拉尔市", adcode: "659002", lng: 81.28053, lat: 40.5472 },
      { name: "阿克苏地区", adcode: "652900", lng: 80.26507, lat: 41.17071 }, { name: "图木舒克市", adcode: "659003", lng: 79.07497, lat: 39.86778 }, { name: "和田地区", adcode: "653200", lng: 79.92165, lat: 37.11441 }, { name: "昆玉市", adcode: "659009", lng: 79.27019, lat: 37.21537 }, { name: "克孜勒苏柯尔克孜自治州", adcode: "653000", lng: 76.16816, lat: 39.71624 },
      { name: "喀什地区", adcode: "653100", lng: 75.99062, lat: 39.47022 },
    ],
  },
  {
    name: "湖北省",
    adcode: "420000",
    lng: 114.34155,
    lat: 30.54622,
    cities: [
      { name: "武汉市", adcode: "420100", lng: 114.30457, lat: 30.59335 }, { name: "黄冈市", adcode: "421100", lng: 114.87243, lat: 30.45372 }, { name: "孝感市", adcode: "420900", lng: 113.95696, lat: 30.91831 }, { name: "鄂州市", adcode: "420700", lng: 114.89491, lat: 30.39146 }, { name: "黄石市", adcode: "420200", lng: 115.039, lat: 30.20108 },
      { name: "咸宁市", adcode: "421200", lng: 114.3226, lat: 29.84135 }, { name: "仙桃市", adcode: "429004", lng: 113.44297, lat: 30.32841 }, { name: "天门市", adcode: "429006", lng: 113.16654, lat: 30.66371 }, { name: "潜江市", adcode: "429005", lng: 112.90028, lat: 30.40195 }, { name: "随州市", adcode: "421300", lng: 113.38232, lat: 31.69027 },
      { name: "荆州市", adcode: "421000", lng: 112.24143, lat: 30.33628 }, { name: "荆门市", adcode: "420800", lng: 112.19901, lat: 31.03544 }, { name: "襄阳市", adcode: "420600", lng: 112.12174, lat: 32.01016 }, { name: "宜昌市", adcode: "420500", lng: 111.28696, lat: 30.69217 }, { name: "神农架林区", adcode: "429021", lng: 110.67588, lat: 31.7451 },
      { name: "十堰市", adcode: "420300", lng: 110.79892, lat: 32.62906 }, { name: "恩施土家族苗族自治州", adcode: "422800", lng: 109.48808, lat: 30.2721 },
    ],
  },
  {
    name: "辽宁省",
    adcode: "210000",
    lng: 123.43509,
    lat: 41.83674,
    cities: [
      { name: "沈阳市", adcode: "210100", lng: 123.46467, lat: 41.67758 }, { name: "抚顺市", adcode: "210400", lng: 123.95705, lat: 41.88131 }, { name: "本溪市", adcode: "210500", lng: 123.68505, lat: 41.48692 }, { name: "铁岭市", adcode: "211200", lng: 123.72601, lat: 42.22371 }, { name: "辽阳市", adcode: "211000", lng: 123.23967, lat: 41.2674 },
      { name: "鞍山市", adcode: "210300", lng: 122.99418, lat: 41.10824 }, { name: "阜新市", adcode: "210900", lng: 121.67005, lat: 42.02203 }, { name: "盘锦市", adcode: "211100", lng: 122.17073, lat: 40.71956 }, { name: "营口市", adcode: "210800", lng: 122.21915, lat: 40.62503 }, { name: "锦州市", adcode: "210700", lng: 121.12686, lat: 41.09611 },
      { name: "丹东市", adcode: "210600", lng: 124.35442, lat: 40.00065 }, { name: "朝阳市", adcode: "211300", lng: 120.4888, lat: 41.60186 }, { name: "葫芦岛市", adcode: "211400", lng: 120.83678, lat: 40.71097 }, { name: "大连市", adcode: "210200", lng: 121.61479, lat: 38.91396 },
    ],
  },
  {
    name: "山东省",
    adcode: "370000",
    lng: 117.02072,
    lat: 36.6702,
    cities: [
      { name: "济南市", adcode: "370100", lng: 117.12013, lat: 36.65207 }, { name: "泰安市", adcode: "370900", lng: 117.08696, lat: 36.20178 }, { name: "淄博市", adcode: "370300", lng: 118.05499, lat: 36.81379 }, { name: "聊城市", adcode: "371500", lng: 115.98524, lat: 36.45586 }, { name: "德州市", adcode: "371400", lng: 116.35924, lat: 37.43649 },
      { name: "滨州市", adcode: "371600", lng: 117.97073, lat: 37.38269 }, { name: "济宁市", adcode: "370800", lng: 116.58712, lat: 35.41512 }, { name: "东营市", adcode: "370500", lng: 118.67463, lat: 37.43399 }, { name: "潍坊市", adcode: "370700", lng: 119.16172, lat: 36.70767 }, { name: "枣庄市", adcode: "370400", lng: 117.32376, lat: 34.81086 },
      { name: "菏泽市", adcode: "371700", lng: 115.47965, lat: 35.23431 }, { name: "临沂市", adcode: "371300", lng: 118.35646, lat: 35.10377 }, { name: "日照市", adcode: "371100", lng: 119.52685, lat: 35.41691 }, { name: "青岛市", adcode: "370200", lng: 120.38267, lat: 36.06694 }, { name: "烟台市", adcode: "370600", lng: 121.44776, lat: 37.46455 },
      { name: "威海市", adcode: "371000", lng: 122.12052, lat: 37.51331 },
    ],
  },
  {
    name: "上海市",
    adcode: "310000",
    lng: 121.47367,
    lat: 31.23053,
    cities: [
      { name: "上海市", adcode: "310000", lng: 121.47367, lat: 31.23053 },
    ],
  },
  {
    name: "贵州省",
    adcode: "520000",
    lng: 106.70525,
    lat: 26.60033,
    cities: [
      { name: "贵阳市", adcode: "520100", lng: 106.6282, lat: 26.64669 }, { name: "安顺市", adcode: "520400", lng: 105.9476, lat: 26.2531 }, { name: "黔南布依族苗族自治州", adcode: "522700", lng: 107.5223, lat: 26.25314 }, { name: "黔东南苗族侗族自治州", adcode: "522600", lng: 107.98284, lat: 26.58376 }, { name: "遵义市", adcode: "520300", lng: 107.03192, lat: 27.72193 },
      { name: "毕节市", adcode: "520500", lng: 105.29154, lat: 27.28362 }, { name: "六盘水市", adcode: "520200", lng: 104.83036, lat: 26.59254 }, { name: "黔西南布依族苗族自治州", adcode: "522300", lng: 104.90642, lat: 25.08773 }, { name: "铜仁市", adcode: "520600", lng: 109.18953, lat: 27.73156 },
    ],
  },
  {
    name: "重庆市",
    adcode: "500000",
    lng: 106.55179,
    lat: 29.56268,
    cities: [
      { name: "重庆市", adcode: "500000", lng: 106.55179, lat: 29.56268 },
    ],
  },
  {
    name: "安徽省",
    adcode: "340000",
    lng: 117.33014,
    lat: 31.73456,
    cities: [
      { name: "合肥市", adcode: "340100", lng: 117.22727, lat: 31.82057 }, { name: "六安市", adcode: "341500", lng: 116.51973, lat: 31.73589 }, { name: "铜陵市", adcode: "340700", lng: 117.8113, lat: 30.94521 }, { name: "淮南市", adcode: "340400", lng: 117.0186, lat: 32.58538 }, { name: "滁州市", adcode: "341100", lng: 118.33344, lat: 32.2559 },
      { name: "马鞍山市", adcode: "340500", lng: 118.50685, lat: 31.66877 }, { name: "芜湖市", adcode: "340200", lng: 118.43306, lat: 31.35261 }, { name: "池州市", adcode: "341700", lng: 117.49566, lat: 30.67426 }, { name: "蚌埠市", adcode: "340300", lng: 117.38857, lat: 32.91682 }, { name: "安庆市", adcode: "340800", lng: 117.11535, lat: 30.53183 },
      { name: "宣城市", adcode: "341800", lng: 118.75913, lat: 30.93928 }, { name: "阜阳市", adcode: "341200", lng: 115.81425, lat: 32.89103 }, { name: "宿州市", adcode: "341300", lng: 116.96419, lat: 33.64773 }, { name: "黄山市", adcode: "341000", lng: 118.33764, lat: 29.71489 }, { name: "淮北市", adcode: "340600", lng: 116.79836, lat: 33.95626 },
      { name: "亳州市", adcode: "341600", lng: 115.77859, lat: 33.84629 },
    ],
  },
  {
    name: "福建省",
    adcode: "350000",
    lng: 119.29619,
    lat: 26.10108,
    cities: [
      { name: "福州市", adcode: "350100", lng: 119.29641, lat: 26.07429 }, { name: "宁德市", adcode: "350900", lng: 119.54773, lat: 26.66622 }, { name: "莆田市", adcode: "350300", lng: 119.00766, lat: 25.4542 }, { name: "泉州市", adcode: "350500", lng: 118.67572, lat: 24.87445 }, { name: "三明市", adcode: "350400", lng: 117.63892, lat: 26.26346 },
      { name: "南平市", adcode: "350700", lng: 118.08133, lat: 27.38283 }, { name: "厦门市", adcode: "350200", lng: 118.08891, lat: 24.47963 }, { name: "漳州市", adcode: "350600", lng: 117.6473, lat: 24.5153 }, { name: "龙岩市", adcode: "350800", lng: 117.01736, lat: 25.07588 },
    ],
  },
  {
    name: "湖南省",
    adcode: "430000",
    lng: 112.98295,
    lat: 28.11601,
    cities: [
      { name: "长沙市", adcode: "430100", lng: 112.93888, lat: 28.2283 }, { name: "湘潭市", adcode: "430300", lng: 112.94544, lat: 27.83136 }, { name: "株洲市", adcode: "430200", lng: 113.13278, lat: 27.82886 }, { name: "益阳市", adcode: "430900", lng: 112.35599, lat: 28.55485 }, { name: "娄底市", adcode: "431300", lng: 111.99447, lat: 27.69984 },
      { name: "岳阳市", adcode: "430600", lng: 113.12892, lat: 29.35648 }, { name: "衡阳市", adcode: "430400", lng: 112.57202, lat: 26.89422 }, { name: "常德市", adcode: "430700", lng: 111.69905, lat: 29.03145 }, { name: "邵阳市", adcode: "430500", lng: 111.46786, lat: 27.23953 }, { name: "永州市", adcode: "431100", lng: 111.61348, lat: 26.41986 },
      { name: "郴州市", adcode: "431000", lng: 113.01552, lat: 25.77012 }, { name: "张家界市", adcode: "430800", lng: 110.47889, lat: 29.11734 }, { name: "怀化市", adcode: "431200", lng: 110.0016, lat: 27.56981 }, { name: "湘西土家族苗族自治州", adcode: "433100", lng: 109.67334, lat: 28.21598 },
    ],
  },
  {
    name: "海南省",
    adcode: "460000",
    lng: 110.34878,
    lat: 20.01864,
    cities: [
      { name: "海口市", adcode: "460100", lng: 110.20016, lat: 20.04632 }, { name: "定安县", adcode: "469021", lng: 110.358, lat: 19.68122 }, { name: "澄迈县", adcode: "469023", lng: 110.0075, lat: 19.73888 }, { name: "临高县", adcode: "469024", lng: 109.69051, lat: 19.91202 }, { name: "文昌市", adcode: "469005", lng: 110.79747, lat: 19.54423 },
      { name: "屯昌县", adcode: "469022", lng: 110.10167, lat: 19.35166 }, { name: "琼海市", adcode: "469002", lng: 110.47452, lat: 19.25911 }, { name: "儋州市", adcode: "460400", lng: 109.58081, lat: 19.52095 }, { name: "琼中黎族苗族自治县", adcode: "469030", lng: 109.83842, lat: 19.03327 }, { name: "白沙黎族自治县", adcode: "469025", lng: 109.4429, lat: 19.22164 },
      { name: "万宁市", adcode: "469006", lng: 110.39261, lat: 18.7937 }, { name: "昌江黎族自治县", adcode: "469026", lng: 109.05578, lat: 19.29814 }, { name: "五指山市", adcode: "469001", lng: 109.51678, lat: 18.77483 }, { name: "保亭黎族苗族自治县", adcode: "469029", lng: 109.70028, lat: 18.64034 }, { name: "陵水黎族自治县", adcode: "469028", lng: 110.03755, lat: 18.50605 },
      { name: "乐东黎族自治县", adcode: "469027", lng: 109.17338, lat: 18.75006 }, { name: "东方市", adcode: "469007", lng: 108.65183, lat: 19.09519 }, { name: "三亚市", adcode: "460200", lng: 109.51171, lat: 18.25286 }, { name: "三沙市", adcode: "460300", lng: 112.33865, lat: 16.831 },
    ],
  },
  {
    name: "江苏省",
    adcode: "320000",
    lng: 118.76356,
    lat: 32.06138,
    cities: [
      { name: "南京市", adcode: "320100", lng: 118.79662, lat: 32.05934 }, { name: "镇江市", adcode: "321100", lng: 119.42444, lat: 32.18814 }, { name: "扬州市", adcode: "321000", lng: 119.41283, lat: 32.3944 }, { name: "常州市", adcode: "320400", lng: 119.97409, lat: 31.81131 }, { name: "泰州市", adcode: "321200", lng: 119.92288, lat: 32.45669 },
      { name: "无锡市", adcode: "320200", lng: 120.31189, lat: 31.49106 }, { name: "淮安市", adcode: "320800", lng: 119.11317, lat: 33.5515 }, { name: "苏州市", adcode: "320500", lng: 120.58529, lat: 31.29976 }, { name: "盐城市", adcode: "320900", lng: 120.16263, lat: 33.34818 }, { name: "南通市", adcode: "320600", lng: 120.89452, lat: 31.98127 },
      { name: "宿迁市", adcode: "321300", lng: 118.27523, lat: 33.96319 }, { name: "徐州市", adcode: "320300", lng: 117.28375, lat: 34.20422 }, { name: "连云港市", adcode: "320700", lng: 119.22149, lat: 34.59664 },
    ],
  },
  {
    name: "青海省",
    adcode: "630000",
    lng: 101.78048,
    lat: 36.62254,
    cities: [
      { name: "西宁市", adcode: "630100", lng: 101.75825, lat: 36.67603 }, { name: "海东市", adcode: "630200", lng: 102.41064, lat: 36.47345 }, { name: "海北藏族自治州", adcode: "632200", lng: 100.90094, lat: 36.95461 }, { name: "海南藏族自治州", adcode: "632500", lng: 100.62265, lat: 36.2964 }, { name: "黄南藏族自治州", adcode: "632300", lng: 102.0154, lat: 35.51932 },
      { name: "果洛藏族自治州", adcode: "632600", lng: 100.24516, lat: 34.47218 }, { name: "海西蒙古族藏族自治州", adcode: "632800", lng: 97.33197, lat: 37.34811 }, { name: "玉树藏族自治州", adcode: "632700", lng: 97.00629, lat: 33.00631 },
    ],
  },
  {
    name: "广西壮族自治区",
    adcode: "450000",
    lng: 108.32754,
    lat: 22.81666,
    cities: [
      { name: "南宁市", adcode: "450100", lng: 108.36641, lat: 22.8177 }, { name: "钦州市", adcode: "450700", lng: 108.65435, lat: 21.98089 }, { name: "崇左市", adcode: "451400", lng: 107.36497, lat: 22.37714 }, { name: "防城港市", adcode: "450600", lng: 108.35467, lat: 21.68673 }, { name: "贵港市", adcode: "450800", lng: 109.5989, lat: 23.11182 },
      { name: "来宾市", adcode: "451300", lng: 109.22124, lat: 23.75011 }, { name: "北海市", adcode: "450500", lng: 109.12025, lat: 21.4813 }, { name: "河池市", adcode: "451200", lng: 108.63639, lat: 24.48523 }, { name: "玉林市", adcode: "450900", lng: 110.18097, lat: 22.654 }, { name: "柳州市", adcode: "450200", lng: 109.42807, lat: 24.32644 },
      { name: "百色市", adcode: "451000", lng: 106.61869, lat: 23.90307 }, { name: "梧州市", adcode: "450400", lng: 111.27902, lat: 23.47673 }, { name: "桂林市", adcode: "450300", lng: 110.17975, lat: 25.23561 }, { name: "贺州市", adcode: "451100", lng: 111.56722, lat: 24.40418 },
    ],
  },
  {
    name: "宁夏回族自治区",
    adcode: "640000",
    lng: 106.25889,
    lat: 38.47227,
    cities: [
      { name: "银川市", adcode: "640100", lng: 106.23098, lat: 38.48778 }, { name: "吴忠市", adcode: "640300", lng: 106.19861, lat: 37.99775 }, { name: "石嘴山市", adcode: "640200", lng: 106.38279, lat: 38.98463 }, { name: "中卫市", adcode: "640500", lng: 105.19677, lat: 37.50019 }, { name: "固原市", adcode: "640400", lng: 106.24267, lat: 36.01628 },
    ],
  },
  {
    name: "浙江省",
    adcode: "330000",
    lng: 120.15257,
    lat: 30.26662,
    cities: [
      { name: "杭州市", adcode: "330100", lng: 120.2099, lat: 30.24657 }, { name: "绍兴市", adcode: "330600", lng: 120.58289, lat: 30.05155 }, { name: "湖州市", adcode: "330500", lng: 120.08688, lat: 30.89418 }, { name: "嘉兴市", adcode: "330400", lng: 120.75562, lat: 30.74681 }, { name: "金华市", adcode: "330700", lng: 119.64727, lat: 29.07919 },
      { name: "宁波市", adcode: "330200", lng: 121.62454, lat: 29.86026 }, { name: "衢州市", adcode: "330800", lng: 118.85931, lat: 28.97023 }, { name: "舟山市", adcode: "330900", lng: 122.2074, lat: 29.98558 }, { name: "丽水市", adcode: "331100", lng: 119.92325, lat: 28.46769 }, { name: "台州市", adcode: "331000", lng: 121.42079, lat: 28.65572 },
      { name: "温州市", adcode: "330300", lng: 120.69928, lat: 27.99385 },
    ],
  },
  {
    name: "河北省",
    adcode: "130000",
    lng: 114.5304,
    lat: 38.03771,
    cities: [
      { name: "石家庄市", adcode: "130100", lng: 114.51498, lat: 38.04201 }, { name: "衡水市", adcode: "131100", lng: 115.66899, lat: 37.73937 }, { name: "邢台市", adcode: "130500", lng: 114.49742, lat: 37.06023 }, { name: "保定市", adcode: "130600", lng: 115.46452, lat: 38.87448 }, { name: "邯郸市", adcode: "130400", lng: 114.53915, lat: 36.62585 },
      { name: "沧州市", adcode: "130900", lng: 116.83871, lat: 38.30468 }, { name: "廊坊市", adcode: "131000", lng: 116.68355, lat: 39.5383 }, { name: "张家口市", adcode: "130700", lng: 114.8859, lat: 40.76893 }, { name: "唐山市", adcode: "130200", lng: 118.18015, lat: 39.63068 }, { name: "承德市", adcode: "130800", lng: 117.96275, lat: 40.95294 },
      { name: "秦皇岛市", adcode: "130300", lng: 119.52022, lat: 39.88824 },
    ],
  },
  {
    name: "香港特别行政区",
    adcode: "810000",
    lng: 114.17071,
    lat: 22.27835,
    cities: [
      { name: "湾仔区", adcode: "810002", lng: 114.18292, lat: 22.27639 }, { name: "中西区", adcode: "810001", lng: 114.15437, lat: 22.28198 }, { name: "油尖旺区", adcode: "810005", lng: 114.17333, lat: 22.3117 }, { name: "南区", adcode: "810004", lng: 114.16001, lat: 22.2459 }, { name: "东区", adcode: "810003", lng: 114.226, lat: 22.27969 },
      { name: "深水埗区", adcode: "810006", lng: 114.16324, lat: 22.33385 }, { name: "观塘区", adcode: "810009", lng: 114.21405, lat: 22.32084 }, { name: "黄大仙区", adcode: "810008", lng: 114.20389, lat: 22.33632 }, { name: "葵青区", adcode: "810017", lng: 114.13932, lat: 22.36388 }, { name: "西贡区", adcode: "810015", lng: 114.26465, lat: 22.31421 },
      { name: "荃湾区", adcode: "810010", lng: 114.12108, lat: 22.36831 }, { name: "沙田区", adcode: "810016", lng: 114.19537, lat: 22.37953 }, { name: "大埔区", adcode: "810014", lng: 114.17174, lat: 22.44565 }, { name: "元朗区", adcode: "810012", lng: 114.03244, lat: 22.44143 }, { name: "离岛区", adcode: "810018", lng: 113.94612, lat: 22.28641 },
      { name: "屯门区", adcode: "810011", lng: 113.97657, lat: 22.39384 }, { name: "北区", adcode: "810013", lng: 114.14736, lat: 22.4961 },
    ],
  },
  {
    name: "澳门特别行政区",
    adcode: "820000",
    lng: 113.54308,
    lat: 22.18693,
    cities: [
      { name: "风顺堂区", adcode: "820005", lng: 113.54193, lat: 22.18737 }, { name: "望德堂区", adcode: "820003", lng: 113.55018, lat: 22.19372 }, { name: "大堂区", adcode: "820004", lng: 113.55365, lat: 22.18854 }, { name: "花王堂区", adcode: "820002", lng: 113.54896, lat: 22.19921 }, { name: "花地玛堂区", adcode: "820001", lng: 113.5529, lat: 22.20787 },
      { name: "嘉模堂区", adcode: "820006", lng: 113.5587, lat: 22.15376 }, { name: "路氹填海区", adcode: "820007", lng: 113.5696, lat: 22.13663 }, { name: "圣方济各堂区", adcode: "820008", lng: 113.55995, lat: 22.12349 },
    ],
  },
  {
    name: "甘肃省",
    adcode: "620000",
    lng: 103.82678,
    lat: 36.06063,
    cities: [
      { name: "兰州市", adcode: "620100", lng: 103.83423, lat: 36.0608 }, { name: "白银市", adcode: "620400", lng: 104.13887, lat: 36.54512 }, { name: "临夏回族自治州", adcode: "622900", lng: 103.21039, lat: 35.60179 }, { name: "定西市", adcode: "621100", lng: 104.59234, lat: 35.60795 }, { name: "甘南藏族自治州", adcode: "623000", lng: 102.91174, lat: 34.98327 },
      { name: "武威市", adcode: "620600", lng: 102.63782, lat: 37.92898 }, { name: "天水市", adcode: "620500", lng: 105.75197, lat: 34.57451 }, { name: "平凉市", adcode: "620800", lng: 106.66491, lat: 35.54242 }, { name: "金昌市", adcode: "620300", lng: 102.18797, lat: 38.52147 }, { name: "陇南市", adcode: "621200", lng: 104.9603, lat: 33.37017 },
      { name: "庆阳市", adcode: "621000", lng: 107.64343, lat: 35.70946 }, { name: "张掖市", adcode: "620700", lng: 100.44986, lat: 38.92477 }, { name: "酒泉市", adcode: "620900", lng: 98.49432, lat: 39.73342 }, { name: "嘉峪关市", adcode: "620200", lng: 98.2882, lat: 39.77325 },
    ],
  },
  {
    name: "四川省",
    adcode: "510000",
    lng: 104.07645,
    lat: 30.6517,
    cities: [
      { name: "成都市", adcode: "510100", lng: 104.0663, lat: 30.57296 }, { name: "德阳市", adcode: "510600", lng: 104.3978, lat: 31.12745 }, { name: "眉山市", adcode: "511400", lng: 103.84842, lat: 30.07711 }, { name: "资阳市", adcode: "512000", lng: 104.62726, lat: 30.12924 }, { name: "绵阳市", adcode: "510700", lng: 104.67913, lat: 31.46767 },
      { name: "雅安市", adcode: "511800", lng: 103.04154, lat: 30.01 }, { name: "乐山市", adcode: "511100", lng: 103.76609, lat: 29.55228 }, { name: "遂宁市", adcode: "510900", lng: 105.5926, lat: 30.53268 }, { name: "内江市", adcode: "511000", lng: 105.05799, lat: 29.58021 }, { name: "自贡市", adcode: "510300", lng: 104.77931, lat: 29.33924 },
      { name: "南充市", adcode: "511300", lng: 106.11056, lat: 30.83723 }, { name: "甘孜藏族自治州", adcode: "513300", lng: 101.96231, lat: 30.04952 }, { name: "宜宾市", adcode: "511500", lng: 104.64283, lat: 28.75235 }, { name: "阿坝藏族羌族自治州", adcode: "513200", lng: 102.2245, lat: 31.89943 }, { name: "泸州市", adcode: "510500", lng: 105.44187, lat: 28.87098 },
      { name: "广安市", adcode: "511600", lng: 106.63265, lat: 30.45635 }, { name: "广元市", adcode: "510800", lng: 105.844, lat: 32.43577 }, { name: "巴中市", adcode: "511900", lng: 106.74755, lat: 31.86785 }, { name: "达州市", adcode: "511700", lng: 107.46778, lat: 31.20928 }, { name: "凉山彝族自治州", adcode: "513400", lng: 102.26771, lat: 27.8814 },
      { name: "攀枝花市", adcode: "510400", lng: 101.72912, lat: 26.55864 },
    ],
  },
  {
    name: "天津市",
    adcode: "120000",
    lng: 117.20151,
    lat: 39.08532,
    cities: [
      { name: "天津市", adcode: "120000", lng: 117.20151, lat: 39.08532 },
    ],
  },
  {
    name: "山西省",
    adcode: "140000",
    lng: 112.57878,
    lat: 37.81395,
    cities: [
      { name: "太原市", adcode: "140100", lng: 112.54966, lat: 37.87045 }, { name: "晋中市", adcode: "140700", lng: 112.75263, lat: 37.68801 }, { name: "忻州市", adcode: "140900", lng: 112.73415, lat: 38.41596 }, { name: "阳泉市", adcode: "140300", lng: 113.58043, lat: 37.85709 }, { name: "吕梁市", adcode: "141100", lng: 111.14454, lat: 37.519 },
      { name: "朔州市", adcode: "140600", lng: 112.43291, lat: 39.33173 }, { name: "长治市", adcode: "140400", lng: 113.11739, lat: 36.19514 }, { name: "临汾市", adcode: "141000", lng: 111.51931, lat: 36.08858 }, { name: "晋城市", adcode: "140500", lng: 112.85202, lat: 35.49132 }, { name: "大同市", adcode: "140200", lng: 113.36675, lat: 40.09711 },
      { name: "运城市", adcode: "140800", lng: 111.00705, lat: 35.02667 },
    ],
  },
  {
    name: "陕西省",
    adcode: "610000",
    lng: 108.95394,
    lat: 34.26661,
    cities: [
      { name: "西安市", adcode: "610100", lng: 108.93964, lat: 34.34321 }, { name: "咸阳市", adcode: "610400", lng: 108.70884, lat: 34.3299 }, { name: "渭南市", adcode: "610500", lng: 109.47096, lat: 34.52063 }, { name: "铜川市", adcode: "610200", lng: 108.94512, lat: 34.89713 }, { name: "商洛市", adcode: "611000", lng: 109.91865, lat: 33.87336 },
      { name: "宝鸡市", adcode: "610300", lng: 107.23768, lat: 34.36286 }, { name: "安康市", adcode: "610900", lng: 109.02902, lat: 32.68543 }, { name: "汉中市", adcode: "610700", lng: 107.02319, lat: 33.06637 }, { name: "延安市", adcode: "610600", lng: 109.49468, lat: 36.65011 }, { name: "榆林市", adcode: "610800", lng: 109.7341, lat: 38.28576 },
    ],
  },
  {
    name: "西藏自治区",
    adcode: "540000",
    lng: 91.11745,
    lat: 29.64869,
    cities: [
      { name: "拉萨市", adcode: "540100", lng: 91.17192, lat: 29.65349 }, { name: "山南市", adcode: "540500", lng: 91.77143, lat: 29.23772 }, { name: "日喀则市", adcode: "540200", lng: 88.88042, lat: 29.26684 }, { name: "那曲市", adcode: "540600", lng: 92.05151, lat: 31.4779 }, { name: "林芝市", adcode: "540400", lng: 94.36144, lat: 29.64875 },
      { name: "昌都市", adcode: "540300", lng: 97.17042, lat: 31.14288 }, { name: "阿里地区", adcode: "542500", lng: 80.10579, lat: 32.50099 },
    ],
  },
  {
    name: "云南省",
    adcode: "530000",
    lng: 102.70937,
    lat: 25.04643,
    cities: [
      { name: "昆明市", adcode: "530100", lng: 102.83367, lat: 24.88149 }, { name: "玉溪市", adcode: "530400", lng: 102.52667, lat: 24.34679 }, { name: "楚雄彝族自治州", adcode: "532300", lng: 101.5283, lat: 25.04568 }, { name: "曲靖市", adcode: "530300", lng: 103.79629, lat: 25.49087 }, { name: "红河哈尼族彝族自治州", adcode: "532500", lng: 103.37487, lat: 23.36313 },
      { name: "文山壮族苗族自治州", adcode: "532600", lng: 104.21567, lat: 23.40098 }, { name: "大理白族自治州", adcode: "532900", lng: 100.26761, lat: 25.60655 }, { name: "昭通市", adcode: "530600", lng: 103.71708, lat: 27.33818 }, { name: "临沧市", adcode: "530900", lng: 100.08884, lat: 23.88417 }, { name: "普洱市", adcode: "530800", lng: 100.96601, lat: 22.82523 },
      { name: "丽江市", adcode: "530700", lng: 100.22594, lat: 26.85516 }, { name: "保山市", adcode: "530500", lng: 99.16149, lat: 25.11202 }, { name: "西双版纳傣族自治州", adcode: "532800", lng: 100.797, lat: 22.00904 }, { name: "怒江傈僳族自治州", adcode: "533300", lng: 98.8566, lat: 25.81755 }, { name: "德宏傣族景颇族自治州", adcode: "533100", lng: 98.58562, lat: 24.43315 },
      { name: "迪庆藏族自治州", adcode: "533400", lng: 99.70211, lat: 27.81915 },
    ],
  },
  {
    name: "台湾省",
    adcode: "710000",
    lng: 121.50906,
    lat: 25.04433,
    cities: [
      { name: "新北市", adcode: "710300", lng: 121.4696, lat: 25.00961 }, { name: "台北市", adcode: "710100", lng: 121.56812, lat: 25.0346 }, { name: "桃园市", adcode: "710600", lng: 121.30505, lat: 24.9902 }, { name: "基隆市", adcode: "711600", lng: 121.74826, lat: 25.12904 }, { name: "宜兰县", adcode: "711700", lng: 121.76699, lat: 24.72786 },
      { name: "新竹县", adcode: "710700", lng: 121.01687, lat: 24.82411 }, { name: "新竹市", adcode: "711200", lng: 120.97276, lat: 24.8039 }, { name: "苗栗县", adcode: "711100", lng: 120.82452, lat: 24.56195 }, { name: "花莲县", adcode: "711000", lng: 121.6235, lat: 23.98852 }, { name: "台中市", adcode: "710400", lng: 120.65055, lat: 24.159 },
      { name: "彰化县", adcode: "711300", lng: 120.54825, lat: 24.07277 }, { name: "南投县", adcode: "710900", lng: 120.69428, lat: 23.90001 }, { name: "云林县", adcode: "711400", lng: 120.53017, lat: 23.69639 }, { name: "嘉义市", adcode: "711800", lng: 120.45766, lat: 23.47889 }, { name: "嘉义县", adcode: "711900", lng: 120.29687, lat: 23.45622 },
      { name: "澎湖县", adcode: "712000", lng: 119.57084, lat: 23.56742 }, { name: "台东县", adcode: "711500", lng: 121.15422, lat: 22.75302 }, { name: "台南市", adcode: "710500", lng: 120.18914, lat: 22.98978 }, { name: "屏东县", adcode: "710800", lng: 120.49175, lat: 22.68021 }, { name: "高雄市", adcode: "710200", lng: 120.31598, lat: 22.61788 },
    ],
  },
  {
    name: "北京市",
    adcode: "110000",
    lng: 116.40739,
    lat: 39.90418,
    cities: [
      { name: "北京市", adcode: "110000", lng: 116.40739, lat: 39.90418 },
    ],
  },
  {
    name: "吉林省",
    adcode: "220000",
    lng: 125.3258,
    lat: 43.89694,
    cities: [
      { name: "长春市", adcode: "220100", lng: 125.32364, lat: 43.817 }, { name: "吉林市", adcode: "220200", lng: 126.54972, lat: 43.83813 }, { name: "四平市", adcode: "220300", lng: 124.3506, lat: 43.16676 }, { name: "辽源市", adcode: "220400", lng: 125.14468, lat: 42.88796 }, { name: "松原市", adcode: "220700", lng: 124.82532, lat: 45.14191 },
      { name: "白山市", adcode: "220600", lng: 126.41427, lat: 41.94413 }, { name: "通化市", adcode: "220500", lng: 125.93972, lat: 41.72831 }, { name: "白城市", adcode: "220800", lng: 122.8381, lat: 45.62013 }, { name: "延边朝鲜族自治州", adcode: "222400", lng: 129.47061, lat: 42.90943 },
    ],
  },
  {
    name: "江西省",
    adcode: "360000",
    lng: 115.81659,
    lat: 28.63723,
    cities: [
      { name: "南昌市", adcode: "360100", lng: 115.85797, lat: 28.68298 }, { name: "抚州市", adcode: "361000", lng: 116.35805, lat: 27.94898 }, { name: "九江市", adcode: "360400", lng: 115.95356, lat: 29.66116 }, { name: "鹰潭市", adcode: "360600", lng: 117.03953, lat: 28.27209 }, { name: "新余市", adcode: "360500", lng: 114.91666, lat: 27.81855 },
      { name: "景德镇市", adcode: "360200", lng: 117.18489, lat: 29.2744 }, { name: "宜春市", adcode: "360900", lng: 114.41683, lat: 27.81624 }, { name: "吉安市", adcode: "360800", lng: 114.96681, lat: 27.09124 }, { name: "上饶市", adcode: "361100", lng: 117.94306, lat: 28.45513 }, { name: "萍乡市", adcode: "360300", lng: 113.88715, lat: 27.65872 },
      { name: "赣州市", adcode: "360700", lng: 114.93349, lat: 25.83114 },
    ],
  },
]

const NORMALIZED = new Map(PROVINCE_CITIES.map((item) => [normalize(item.name), item]))

function normalize(value) {
  return String(value || '')
    .trim()
    .replace(/特别行政区|维吾尔自治区|壮族自治区|回族自治区|自治区|省|市$/g, '')
}

/** 找到目的地所属省份；直辖市直接按自身处理。 */
export function provinceOf(name) {
  const text = String(name || '').trim()
  if (!text) return null
  const exact = PROVINCE_CITIES.find((item) => text.includes(item.name))
  if (exact) return exact
  const key = normalize(text)
  if (NORMALIZED.has(key)) return NORMALIZED.get(key)
  // 用户常只写简称（「新疆」「内蒙」「北京」）：简称要么是省份名去掉民族/后缀后的
  // 前缀，要么是省份名的前缀（「广西」→「广西壮族自治区」），两种都算命中。
  if (key.length >= 2) {
    const shortName = (value) => String(value || '').replace(/维吾尔|壮族|回族|蒙古|哈萨克|柯尔克孜|土家|苗族|藏族|壮族/g, '')
    const prefixed = PROVINCE_CITIES.find(
      (item) => shortName(normalize(item.name)).startsWith(shortName(key)) ||
        normalize(item.name).startsWith(key),
    )
    if (prefixed) return prefixed
  }
  for (const item of PROVINCE_CITIES) {
    const short = normalize(item.name)
    if (short && text.includes(short)) return item
  }
  return null
}

/** 省级目的地的保底城市：省会优先，再按离省中心由近到远。 */
export function fallbackCities(province, limit = 8) {
  if (!province) return []
  const cities = province.cities || []
  if (cities.length <= limit) return cities.slice()
  return cities.slice(0, limit)
}
