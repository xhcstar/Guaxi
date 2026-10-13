<template>
  <div class="tb-root">
    <n-tabs type="line" size="small" class="tb-tabs" pane-class="tb-pane">
      <n-tab-pane name="calc" tab="计算器">
        <template #tab><span class="tw-tab"><Icon :icon="calcIcon" width="15" height="15" />计算器</span></template>
        <div class="tb-sec">
          <div class="tb-h">进制 / 编码转换</div>
          <n-input v-model:value="numInput" size="small" placeholder="输入 0x1F / 0b1010 / 31 / 0o37" @input="convertNum" style="max-width:320px" />
          <table class="tb-kv" v-if="numOut">
            <tbody>
              <tr><td>HEX</td><td>{{ numOut.hex }}</td></tr>
              <tr><td>DEC</td><td>{{ numOut.dec }}</td></tr>
              <tr><td>OCT</td><td>{{ numOut.oct }}</td></tr>
              <tr><td>BIN</td><td>{{ numOut.bin }}</td></tr>
              <tr><td>ASCII</td><td>{{ numOut.ascii }}</td></tr>
            </tbody>
          </table>
        </div>
        <div class="tb-sec">
          <div class="tb-h">程序员计算器（位运算）</div>
          <div class="tb-row">
            <n-input v-model:value="opA" size="small" placeholder="操作数 A" style="width:150px" />
            <n-select v-model:value="op" size="small" :options="opOptions" style="width:90px" />
            <n-input v-model:value="opB" size="small" placeholder="操作数 B" style="width:150px" />
            <n-button size="small" type="primary" @click="calcOp">=</n-button>
            <span class="tb-res" v-if="opResult">{{ opResult }}</span>
          </div>
        </div>
        <div class="tb-sec">
          <div class="tb-h">时间戳互转</div>
          <div class="tb-row">
            <n-input v-model:value="epochInput" size="small" placeholder="epoch 秒或毫秒" style="width:180px" @input="epochToDate" />
            <span class="tb-res">{{ epochOut }}</span>
          </div>
          <div class="tb-row">
            <n-input v-model:value="dateInput" size="small" placeholder="2026-09-23 12:00:00" style="width:220px" @input="dateToEpoch" />
            <span class="tb-res">{{ dateOut }}</span>
          </div>
        </div>
        <div class="tb-sec">
          <div class="tb-h">IP 地址转换</div>
          <div class="tb-row">
            <n-input v-model:value="ipInput" size="small" placeholder="IPv4: 192.168.1.10 / 3232235786 / 0xC0A8010A / CIDR 10.0.0.0/24　·　IPv6: 2001:db8::1 / ::1 / fe80::1 / CIDR 2001:db8::/32" style="width:660px" @input="convertIp" />
          </div>
          <table class="tb-kv" v-if="ipOut">
            <tbody>
              <tr v-for="(v, k) in ipOut" :key="k"><td>{{ k }}</td><td>{{ v }}</td></tr>
            </tbody>
          </table>
        </div>
      </n-tab-pane>

      <n-tab-pane name="crypto" tab="证书/ASN.1" class="cx-pane">
        <template #tab><span class="tw-tab"><Icon :icon="keyIcon" width="15" height="15" />证书/密钥</span></template>
        <n-tabs type="segment" size="small" class="cx-tabs" v-model:value="cxTab">
          <n-tab-pane name="parse" tab="解析/导入">
            <div class="tb-sec">
              <div class="tb-h">
                证书 / 私钥 / 公钥 / CSR / PFX / ASN.1 解析
                <n-tooltip trigger="hover"><template #trigger><Icon :icon="helpIcon" width="13" height="13" class="tb-help" /></template>
                可直接选文件（自动识别 PEM/DER/PFX，PFX 填密码即拆包），也可粘贴 PEM 文本、base64 或 hex 编码的 DER。
                </n-tooltip>
              </div>
              <div class="tb-row">
                <n-tooltip trigger="hover"><template #trigger>
                  <button class="tb-ib" @click="pickCryptoFile"><Icon :icon="folderIcon" width="15" height="15" /></button>
                </template>选择文件（.crt / .pem / .key / .csr / .pfx / .p12 / .der）</n-tooltip>
                <n-tooltip trigger="hover"><template #trigger><n-input v-model:value="pfxPassword" size="small" placeholder="PFX/P12 密码（选了 PFX 才需要）" style="width:220px" /></template>如果选的是 .pfx/.p12 文件，在这里填其保护密码；其它类型留空</n-tooltip>
                <span class="tb-res" v-if="loadedKind">识别: {{ loadedKind }}</span>
              </div>
              <n-input v-model:value="cryptoInput" type="textarea" size="small" :rows="6" placeholder="或粘贴 PEM (-----BEGIN...)、base64 / hex 编码的 DER" />
              <div class="tb-row" style="margin-top:8px">
                <n-tooltip trigger="hover"><template #trigger>
                  <button class="tb-ib primary lg" :disabled="cryptoLoading" @click="runCrypto"><Icon :icon="cryptoLoading ? refreshIcon : playIcon" width="17" height="17" :class="{ 'tb-spin': cryptoLoading }" /></button>
                </template>解析上面的内容</n-tooltip>
                <n-tooltip trigger="hover"><template #trigger>
                  <button class="tb-ib" :disabled="!cryptoInput" @click="openView('输入内容详情', cryptoInput)"><Icon :icon="eyeIcon" width="15" height="15" /></button>
                </template>查看详情（字段 / PEM / DER base64，可复制）</n-tooltip>
                <n-tooltip trigger="hover"><template #trigger>
                  <button class="tb-ib" :disabled="!cryptoReport" @click="sendToEditor(cryptoAsText)"><Icon :icon="uploadIcon" width="15" height="15" /></button>
                </template>把解析结果发送到主界面编辑器</n-tooltip>
                <span class="tb-res" v-if="cryptoReport">{{ cryptoReport.kind }}</span>
                <span v-if="error" class="tb-err">{{ error }}</span>
                <span v-if="viewCopied" class="tb-res">已复制</span>
              </div>
              <table class="tb-kv" v-if="cryptoReport && cryptoReport.summary.length">
                <tbody><tr v-for="(kv, i) in cryptoReport.summary" :key="i"><td>{{ kv[0] }}</td><td>{{ kv[1] }}</td></tr></tbody>
              </table>
              <pre class="tb-out" v-if="cryptoReport">{{ cryptoReport.asn1 }}</pre>
              <template v-if="pfxOut">
                <div class="tb-h" style="margin-top:10px">PFX 内容</div>
                <div class="tb-row">
                  <n-tooltip trigger="hover"><template #trigger><button class="tb-ib" @click="openView('PFX 中的私钥', (pfxOut.match(/-----BEGIN[\s\S]*?PRIVATE KEY-----[\s\S]*?-----END[^-]*-----/) || [''])[0])"><Icon :icon="keyIcon" width="15" height="15" /></button></template>查看 PFX 中的私钥</n-tooltip>
                  <n-tooltip trigger="hover"><template #trigger><button class="tb-ib" @click="openView('PFX 中的证书链', (pfxOut.match(/-----BEGIN CERTIFICATE-----[\s\S]*?-----END CERTIFICATE-----/g) || []).join('\n'))"><Icon :icon="certificateIcon" width="15" height="15" /></button></template>查看 PFX 中的证书链</n-tooltip>
                </div>
                <pre class="tb-out">{{ pfxOut }}</pre>
              </template>
            </div>
          </n-tab-pane>

          <n-tab-pane name="chain" tab="证书链/签发">
            <div class="tb-sec">
              <div class="tb-h">CA 库 · 四类（RSA / ECC / SM2 / PQC）—— 每行含证书与私钥，可查看/导出</div>
              <template v-for="g in caGroups" :key="g.cat">
                <div v-if="g.items.length" class="tb-grp">
                  <Icon :icon="folderIcon" width="13" height="13" class="tb-grp-ico" />{{ g.label }}
                  <span class="tb-grp-n">{{ g.items.length }}</span>
                </div>
                <table class="tb-table" v-if="g.items.length">
                  <thead><tr><th>名称</th><th>类型</th><th>主题</th><th>CRL#</th><th>吊销</th><th>操作</th></tr></thead>
                  <tbody>
                    <tr v-for="c in g.items" :key="c.id">
                      <td>{{ c.name }}</td>
                      <td>{{ c.is_root ? '根' : '中间' }}</td>
                      <td class="tb-res" :title="c.subject_cn">{{ c.subject_cn }}</td>
                      <td>{{ c.crl_number }}</td>
                      <td>{{ c.revoked_count }}</td>
                      <td class="tb-ops">
                        <n-tooltip trigger="hover"><template #trigger>
                          <button class="tb-ib" @click="viewCa(c,'cert')"><Icon :icon="eyeIcon" width="14" height="14" /></button>
                        </template>查看该 CA 的证书详情（字段 / PEM / DER base64，可复制）</n-tooltip>
                        <n-tooltip trigger="hover"><template #trigger>
                          <button class="tb-ib" @click="viewCa(c,'key')"><Icon :icon="keyIcon" width="14" height="14" /></button>
                        </template>查看该 CA 的私钥（字段 / PEM / base64，可复制）</n-tooltip>
                        <n-tooltip trigger="hover"><template #trigger>
                          <button class="tb-ib" @click="exportCa(c)"><Icon :icon="downloadIcon" width="14" height="14" /></button>
                        </template>导出该 CA 的证书(.crt) 与私钥(.key)</n-tooltip>
                        <n-tooltip trigger="hover"><template #trigger>
                          <button class="tb-ib" @click="genCrl(c)"><Icon :icon="banIcon" width="14" height="14" /></button>
                        </template>生成该 CA 的吊销列表 CRL 并保存</n-tooltip>
                        <n-tooltip trigger="hover"><template #trigger>
                          <button class="tb-ib danger" @click="deleteCa(c)"><Icon :icon="trashIcon" width="14" height="14" /></button>
                        </template>从库中删除该 CA（不影响已签发证书）</n-tooltip>
                      </td>
                    </tr>
                  </tbody>
                </table>
              </template>
              <div v-if="!caList.length" class="tb-hint">还没有 CA，先在下面"新建 CA"生成一个根 CA。</div>
              <span v-if="error" class="tb-err">{{ error }}</span>
            </div>

            <div class="tb-sec">
              <div class="tb-h">
                主题信息（新建 CA 与签发终端证书共用）
                <n-tooltip trigger="hover"><template #trigger><Icon :icon="helpIcon" width="13" height="13" class="tb-help" /></template>
                X.509 主题是证书里的身份信息：C=国家(两位代码，如 CN)、ST=省/州、L=城市、O=组织、OU=部门、CN=通用名（服务端证书这里必须是域名，如 server.example.com）。留空则使用默认值。
                </n-tooltip>
              </div>
              <div class="tb-row">
                <n-tooltip trigger="hover"><template #trigger><n-input v-model:value="subjC" size="small" placeholder="国家 C" style="width:110px" /></template>C —— 国家/地区，两位代码，如 CN / US / JP</n-tooltip>
                <n-tooltip trigger="hover"><template #trigger><n-input v-model:value="subjST" size="small" placeholder="省 ST" style="width:120px" /></template>ST —— 省 / 州，如 Shannxi / California</n-tooltip>
                <n-tooltip trigger="hover"><template #trigger><n-input v-model:value="subjL" size="small" placeholder="市 L" style="width:120px" /></template>L —— 城市，如 xi'an / Beijing</n-tooltip>
                <n-tooltip trigger="hover"><template #trigger><n-input v-model:value="subjO" size="small" placeholder="组织 O" style="width:130px" /></template>O —— 组织 / 公司名，如 XA / Acme Inc</n-tooltip>
                <n-tooltip trigger="hover"><template #trigger><n-input v-model:value="subjOU" size="small" placeholder="部门 OU" style="width:120px" /></template>OU —— 部门 / 团队，如 Dev / Ops</n-tooltip>
                <n-tooltip trigger="hover"><template #trigger><n-input-number v-model:value="chainDays" size="small" :min="1" :max="36500" :show-button="false" style="width:110px" /></template>证书有效期（天）。根 CA 常用 3650（10 年），服务端证书常用 365～825</n-tooltip>
                <span class="tb-res">天</span>
              </div>
              <div class="tb-row">
                <n-tooltip trigger="hover"><template #trigger>
                  <n-input v-model:value="subjectText" size="small" placeholder="粘贴 openssl 主题串，如 /C=CN/ST=Shannxi/L=xi'an/O=XA/OU=Dev/CN=Root CA" style="flex:1;min-width:280px" @keyup.enter="parseSubjectString" />
                </template>把 openssl -subj 里那串直接粘进来，会自动拆分填入上面的字段（CN 同时填到"新建 CA"与"签发终端证书"的 CN）</n-tooltip>
                <n-tooltip trigger="hover"><template #trigger>
                  <button class="tb-ib" @click="parseSubjectString"><Icon :icon="wandIcon" width="15" height="15" /></button>
                </template>解析并填入上面的主题字段</n-tooltip>
                <span class="tb-res">{{ subjectHint }}</span>
              </div>
            </div>

            <div class="tb-sec">
              <div class="tb-h">
                新建 CA
                <n-tooltip trigger="hover"><template #trigger><Icon :icon="helpIcon" width="13" height="13" class="tb-help" /></template>
                「上级 CA」留空 = 生成自签的<b>根 CA</b>；选一个同类上级 = 由它签发<b>中间 CA</b>（构成 root → intermediate 链）。算法随类别自动匹配：RSA 选位数、ECC 选曲线。
                </n-tooltip>
              </div>
              <div class="tb-row">
                <n-tooltip trigger="hover"><template #trigger><n-select v-model:value="caCategory" size="small" :options="caCategoryOptions" style="width:130px" /></template>CA 类别：RSA / ECC（本地签发）· SM2 / PQC（需 tlcp 特性 + Tongsuo）</n-tooltip>
                <n-tooltip v-if="caCategory==='rsa'" trigger="hover"><template #trigger><n-select v-model:value="chainBits" size="small" :options="rsaBitsOptions" style="width:160px" /></template>RSA 密钥长度。CA 建议 ≥ 3072 位</n-tooltip>
                <n-tooltip v-if="caCategory==='ecc'" trigger="hover"><template #trigger><n-select v-model:value="chainCurve" size="small" :options="ecCurveOptions" style="width:150px" /></template>椭圆曲线。P-256 通用；P-384 更高强度</n-tooltip>
                <n-tooltip trigger="hover"><template #trigger><n-input v-model:value="caName" size="small" placeholder="CA 名称（库内标识，如 root-ca-ecc）" style="width:210px" /></template>库内显示名，只用于你自己识别，不写进证书</n-tooltip>
                <n-tooltip trigger="hover"><template #trigger><n-input v-model:value="caCn" size="small" placeholder="CA 的 CN，如 Root CA (ECC)" style="width:210px" /></template>CA 证书的 CN，会写进证书主题（可先用上面的"主题串"一键填入）</n-tooltip>
                <n-tooltip trigger="hover"><template #trigger><n-select v-model:value="caParent" size="small" :options="caParentOptions" placeholder="上级 CA（空 = 根 CA）" clearable style="width:200px" /></template>留空 = 自签根 CA；选一个同类 CA = 由它签发中间 CA</n-tooltip>
                <n-tooltip trigger="hover"><template #trigger>
                  <button class="tb-ib primary lg" :disabled="caLoading" @click="createCa"><Icon :icon="caLoading ? refreshIcon : plusIcon" width="17" height="17" :class="{ 'tb-spin': caLoading }" /></button>
                </template>按当前参数生成 CA（证书 + 私钥自动入库）</n-tooltip>
              </div>
              <div v-if="caCategory==='sm2'||caCategory==='pqc'" class="tb-hint">SM2 / PQC 证书需启用 <code>tlcp</code> 特性并安装 Tongsuo（见设计文档 §5.3），默认构建会提示未启用。</div>
            </div>

            <div class="tb-sec">
              <div class="tb-h">
                签发终端证书（server / client）
                <n-tooltip trigger="hover"><template #trigger><Icon :icon="helpIcon" width="13" height="13" class="tb-help" /></template>
                选一个库内 CA 作为<b>签发者</b>，为下面的 CN 签发终端证书。服务端证书的 CN/SAN 必须是<b>客户端实际访问的域名或 IP</b>，否则校验不过；客户端证书用于双向认证。生成的证书+私钥会进下方"终端证书库"。
                </n-tooltip>
              </div>
              <div class="tb-row">
                <n-tooltip trigger="hover"><template #trigger><n-select v-model:value="issueCaId" size="small" :options="caSelectOptions" placeholder="选择签发 CA" style="width:230px" /></template>用哪个 CA 签发（显示类别与根/中间）</n-tooltip>
                <n-tooltip trigger="hover"><template #trigger><n-select v-model:value="leafProfile" size="small" :options="leafProfileOptions" style="width:120px" /></template>服务端 = 含 serverAuth，用于 HTTPS/TLS 服务；客户端 = 含 clientAuth，用于 mTLS 客户端</n-tooltip>
                <n-tooltip trigger="hover"><template #trigger><n-input v-model:value="leafCn" size="small" placeholder="CN，如 server.example.com" style="width:220px" /></template>证书的 CN。服务端证书建议直接写域名（或用 IP）</n-tooltip>
                <n-tooltip trigger="hover"><template #trigger><n-input v-model:value="leafSans" size="small" placeholder="SAN 逗号分隔，如 a.com,www.a.com,10.0.0.1" style="flex:1;min-width:180px" /></template>主题备用名（SAN）。现代浏览器只看 SAN，务必把域名/IP 都列上，逗号分隔</n-tooltip>
                <n-tooltip trigger="hover"><template #trigger>
                  <button class="tb-ib primary lg" :disabled="leafLoading" @click="issueLeaf"><Icon :icon="leafLoading ? refreshIcon : certificateIcon" width="17" height="17" :class="{ 'tb-spin': leafLoading }" /></button>
                </template>用所选 CA 签发终端证书并入库</n-tooltip>
              </div>
              <div class="tb-row" v-if="leafCertOut">
                <n-tooltip trigger="hover"><template #trigger><button class="tb-ib" @click="openView('新签发证书', leafCertOut)"><Icon :icon="eyeIcon" width="15" height="15" /></button></template>查看刚签发的证书详情</n-tooltip>
                <n-tooltip trigger="hover"><template #trigger><button class="tb-ib" @click="openView('新签发私钥', leafKeyOut)"><Icon :icon="keyIcon" width="15" height="15" /></button></template>查看刚签发的私钥</n-tooltip>
                <n-tooltip trigger="hover"><template #trigger><button class="tb-ib" @click="saveText(leafKeyOut,'server.key')"><Icon :icon="saveIcon" width="15" height="15" /></button></template>保存私钥到文件</n-tooltip>
                <n-tooltip trigger="hover"><template #trigger><button class="tb-ib" @click="saveText(leafCertOut,'server.crt')"><Icon :icon="downloadIcon" width="15" height="15" /></button></template>保存证书到文件</n-tooltip>
                <n-tooltip trigger="hover"><template #trigger><button class="tb-ib" @click="sendToEditor('```\n'+leafKeyOut+leafCertOut+'\n```')"><Icon :icon="uploadIcon" width="15" height="15" /></button></template>把证书与私钥发送到主界面编辑器</n-tooltip>
              </div>
              <pre class="tb-out" v-if="leafCertOut">{{ leafKeyOut }}{{ leafCertOut }}</pre>
            </div>

            <div class="tb-sec">
              <div class="tb-h">终端证书库（client / server）—— 每行含证书与私钥，可查看/导出/吊销</div>
              <table class="tb-table" v-if="leafList.length">
                <thead><tr><th>名称</th><th>类型</th><th>签发 CA</th><th>主题</th><th>序列号</th><th>操作</th></tr></thead>
                <tbody>
                  <tr v-for="l in leafList" :key="l.id">
                    <td>{{ l.name }}</td>
                    <td>{{ l.kind === 'server' ? '服务端' : '客户端' }}</td>
                    <td class="tb-res" :title="l.ca_name">{{ l.ca_name }}</td>
                    <td class="tb-res" :title="l.subject_cn">{{ l.subject_cn }}</td>
                    <td>{{ l.serial }}</td>
                    <td class="tb-ops">
                      <n-tooltip trigger="hover"><template #trigger><button class="tb-ib" @click="viewLeaf(l,'cert')"><Icon :icon="eyeIcon" width="14" height="14" /></button></template>查看证书详情（字段 / PEM / base64）</n-tooltip>
                      <n-tooltip trigger="hover"><template #trigger><button class="tb-ib" @click="viewLeaf(l,'key')"><Icon :icon="keyIcon" width="14" height="14" /></button></template>查看配套私钥</n-tooltip>
                      <n-tooltip trigger="hover"><template #trigger><button class="tb-ib" @click="viewLeaf(l,'chain')"><Icon :icon="listIcon" width="14" height="14" /></button></template>查看完整证书链（叶 + 各级 CA）</n-tooltip>
                      <n-tooltip trigger="hover"><template #trigger><button class="tb-ib" @click="exportLeaf(l,'cert')"><Icon :icon="certificateIcon" width="14" height="14" /></button></template>导出证书 .crt</n-tooltip>
                      <n-tooltip trigger="hover"><template #trigger><button class="tb-ib" @click="exportLeaf(l,'key')"><Icon :icon="downloadIcon" width="14" height="14" /></button></template>导出私钥 .key</n-tooltip>
                      <n-tooltip trigger="hover"><template #trigger><button class="tb-ib" @click="exportLeaf(l,'chain')"><Icon :icon="saveIcon" width="14" height="14" /></button></template>导出完整证书链 .chain.pem</n-tooltip>
                      <n-tooltip trigger="hover"><template #trigger><button class="tb-ib danger" @click="revokeCa(l.ca_id, l.serial)"><Icon :icon="banIcon" width="14" height="14" /></button></template>吊销该证书（登记到签发 CA 的吊销列表，随后可生成 CRL）</n-tooltip>
                      <n-tooltip trigger="hover"><template #trigger><button class="tb-ib danger" @click="deleteLeaf(l)"><Icon :icon="trashIcon" width="14" height="14" /></button></template>从库中删除该证书</n-tooltip>
                    </td>
                  </tr>
                </tbody>
              </table>
              <div v-else class="tb-hint">用上面"签发终端证书"生成的 client/server 证书会记录在此，可查看/导出证书、私钥与完整链，也可吊销。</div>
            </div>

            <div class="tb-sec">
              <div class="tb-h">导入已有 CA 到库</div>
              <div class="tb-row">
                <n-input v-model:value="importCaName" size="small" placeholder="名称（库内标识）" style="width:200px" />
                <n-tooltip trigger="hover"><template #trigger>
                  <button class="tb-ib" :class="{ 'tb-ok': !!importCertPem }" @click="pickCaFile('cert')"><Icon :icon="certificateIcon" width="15" height="15" /></button>
                </template>{{ importCertPem ? '已选到 CA 证书（点击重新选择）' : '选择 CA 证书文件（PEM / DER）' }}</n-tooltip>
                <n-tooltip trigger="hover"><template #trigger>
                  <button class="tb-ib" :class="{ 'tb-ok': !!importKeyPem }" @click="pickCaFile('key')"><Icon :icon="keyIcon" width="15" height="15" /></button>
                </template>{{ importKeyPem ? '已选到 CA 私钥（点击重新选择）' : '选择 CA 私钥文件（PKCS#8 PEM）' }}</n-tooltip>
                <n-tooltip trigger="hover"><template #trigger>
                  <button class="tb-ib primary lg" :disabled="!importCertPem || !importKeyPem" @click="importCa"><Icon :icon="uploadIcon" width="17" height="17" /></button>
                </template>把选到的证书+私钥作为 CA 入库（之后可直接用它签发）</n-tooltip>
                <span class="tb-res">{{ importCertPem ? '证书 ✓' : '证书 —' }} · {{ importKeyPem ? '私钥 ✓' : '私钥 —' }}</span>
              </div>
            </div>

            <div class="tb-sec">
              <div class="tb-h">
                独立密钥 / CSR / 签名验签
                <n-tooltip trigger="hover"><template #trigger><Icon :icon="helpIcon" width="13" height="13" class="tb-help" /></template>
                这里生成的是<b>不成套</b>的独立密钥与 CSR，方便单独保存或交给外部 CA 签名。若要做完整证书链，请用上面的 CA 库流程。
                </n-tooltip>
              </div>
              <div class="tb-row">
                <span class="tb-res">密钥</span>
                <n-tooltip trigger="hover"><template #trigger><n-select v-model:value="keyAlgo" size="small" :options="keyAlgoOptions" style="width:170px" /></template>密钥算法。当前库支持 RSA / ECDSA(P-256,P-384) / Ed25519</n-tooltip>
                <n-tooltip v-if="keyAlgo==='rsa'" trigger="hover"><template #trigger><n-select v-model:value="rsaBits" size="small" :options="rsaBitsOptions" style="width:190px" /></template>RSA 位数：越大越安全也越慢，2048 起步，长期用 3072/4096</n-tooltip>
                <n-tooltip v-if="keyAlgo==='ec'" trigger="hover"><template #trigger><n-select v-model:value="ecCurve" size="small" :options="ecCurveOptions" style="width:200px" /></template>椭圆曲线。P-256 最常见；P-384 更高强度（P-521 需要 aws-lc 后端，本机不可用）</n-tooltip>
                <n-tooltip trigger="hover"><template #trigger><button class="tb-ib primary" :class="{ 'tb-spin': keyLoading }" @click="genKey"><Icon :icon="plusIcon" width="15" height="15" /></button></template>生成密钥对（私钥 + 公钥）</n-tooltip>
                <n-tooltip trigger="hover"><template #trigger><button class="tb-ib" :disabled="!genPrivPem" @click="openView('生成的私钥', genPrivPem)"><Icon :icon="eyeIcon" width="15" height="15" /></button></template>查看私钥详情</n-tooltip>
                <n-tooltip trigger="hover"><template #trigger><button class="tb-ib" :disabled="!genPrivPem" @click="saveText(genPrivPem,'key.key')"><Icon :icon="saveIcon" width="15" height="15" /></button></template>保存私钥到文件</n-tooltip>
                <n-tooltip trigger="hover"><template #trigger><button class="tb-ib" :disabled="!genPubPem" @click="saveText(genPubPem,'key.pub')"><Icon :icon="downloadIcon" width="15" height="15" /></button></template>保存公钥到文件</n-tooltip>
                <n-tooltip trigger="hover"><template #trigger><button class="tb-ib" :disabled="!genPrivPem" @click="saveAsset('key', keyAlgo+' key', keyAlgo, genPrivPem, genPubPem)"><Icon :icon="uploadIcon" width="15" height="15" /></button></template>把密钥对存进下方"受管资产库"</n-tooltip>
              </div>
              <pre class="tb-out" v-if="genPrivPem">{{ genPrivPem }}{{ genPubPem }}</pre>
              <div class="tb-row" style="margin-top:6px">
                <span class="tb-res">CSR</span>
                <n-tooltip trigger="hover"><template #trigger><n-input v-model:value="csrCn" size="small" placeholder="CN，如 example.com" style="width:200px" /></template>CSR 的 CN，通常写域名或设备名</n-tooltip>
                <n-tooltip trigger="hover"><template #trigger><n-input v-model:value="csrSans" size="small" placeholder="SAN 逗号分隔（可空）" style="width:240px" /></template>主题备用名，逗号分隔；域名或 IP 都写这里</n-tooltip>
                <n-tooltip trigger="hover"><template #trigger><button class="tb-ib" @click="genCsr"><Icon :icon="wandIcon" width="15" height="15" /></button></template>用上面生成的私钥签发 CSR</n-tooltip>
                <n-tooltip trigger="hover"><template #trigger><button class="tb-ib" :disabled="!csrOut" @click="openView('生成的 CSR', csrOut)"><Icon :icon="eyeIcon" width="15" height="15" /></button></template>查看 CSR 详情</n-tooltip>
                <n-tooltip trigger="hover"><template #trigger><button class="tb-ib" :disabled="!csrOut" @click="saveText(csrOut,'request.csr')"><Icon :icon="saveIcon" width="15" height="15" /></button></template>保存 CSR 到文件</n-tooltip>
                <n-tooltip trigger="hover"><template #trigger><button class="tb-ib" :disabled="!csrOut" @click="saveAsset('csr', csrCn+' csr', '', csrOut, '')"><Icon :icon="uploadIcon" width="15" height="15" /></button></template>把 CSR 存进资产库</n-tooltip>
              </div>
              <pre class="tb-out" v-if="csrOut">{{ csrOut }}</pre>
              <div class="tb-row" style="margin-top:6px">
                <span class="tb-res">签名验签</span>
                <n-tooltip trigger="hover"><template #trigger><n-select v-model:value="signScheme" size="small" :options="signSchemeOptions" style="width:210px" /></template>签名/验签算法。RSA-PSS 比 PKCS#1 v1.5 更安全；ECDSA/Ed25519 签名更短</n-tooltip>
                <n-tooltip trigger="hover"><template #trigger><n-checkbox v-model:checked="signTextInput" size="small">明文输入</n-checkbox></template>勾选 = 待签内容是纯文本；不勾 = 按 base64 解码后再签</n-tooltip>
                <n-tooltip trigger="hover"><template #trigger><button class="tb-ib primary" @click="runSign"><Icon :icon="playIcon" width="15" height="15" /></button></template>用上面的私钥对数据签名</n-tooltip>
                <n-tooltip trigger="hover"><template #trigger><button class="tb-ib" @click="runVerify"><Icon :icon="certificateIcon" width="15" height="15" /></button></template>用上面的公钥验证签名</n-tooltip>
                <span class="tb-res" v-if="signVerifyResult!==null">{{ signVerifyResult ? '✓ 通过' : '✗ 失败' }}</span>
              </div>
              <n-input v-model:value="signKey" type="textarea" size="small" :rows="2" placeholder="签名=私钥 PEM / 验签=公钥 PEM（空=用上面生成的密钥）" />
              <n-input v-model:value="signData" type="textarea" size="small" :rows="2" placeholder="原始数据" />
              <n-input v-model:value="signSig" size="small" placeholder="签名(base64)" />
              <pre class="tb-out" v-if="signOut">{{ signOut }}</pre>
            </div>

            <div class="tb-sec">
              <div class="tb-h">受管资产库 · 私钥 / CSR / CRL（持久化，可查看/导入/导出/删除）</div>
              <div class="tb-row">
                <n-tooltip trigger="hover"><template #trigger><button class="tb-ib" @click="importAsset('key')"><Icon :icon="keyIcon" width="15" height="15" /></button></template>导入现有私钥文件入库</n-tooltip>
                <n-tooltip trigger="hover"><template #trigger><button class="tb-ib" @click="importAsset('csr')"><Icon :icon="fileIcon" width="15" height="15" /></button></template>导入现有 CSR 文件入库</n-tooltip>
                <n-tooltip trigger="hover"><template #trigger><button class="tb-ib" @click="importAsset('crl')"><Icon :icon="banIcon" width="15" height="15" /></button></template>导入现有 CRL 文件入库</n-tooltip>
              </div>
              <table class="tb-table" v-if="assetList.length">
                <thead><tr><th>类型</th><th>名称</th><th>算法</th><th>操作</th></tr></thead>
                <tbody>
                  <tr v-for="a in assetList" :key="a.id">
                    <td>{{ assetKindLabel[a.kind] || a.kind }}</td>
                    <td class="tb-res">{{ a.name }}</td>
                    <td>{{ a.algo }}</td>
                    <td>
                      <n-tooltip trigger="hover"><template #trigger><button class="tb-ib" @click="viewAsset(a)"><Icon :icon="eyeIcon" width="14" height="14" /></button></template>查看详情（字段 / PEM / base64，可复制）</n-tooltip>
                      <n-tooltip trigger="hover"><template #trigger><button class="tb-ib" @click="exportAsset(a)"><Icon :icon="downloadIcon" width="14" height="14" /></button></template>导出到文件</n-tooltip>
                      <n-tooltip trigger="hover"><template #trigger><button class="tb-ib danger" @click="deleteAsset(a)"><Icon :icon="trashIcon" width="14" height="14" /></button></template>从资产库删除</n-tooltip>
                    </td>
                  </tr>
                </tbody>
              </table>
              <div v-else class="tb-hint">生成密钥/CSR 后点"存入库"，或点上方"导入"把现有 key/CSR/CRL 文件纳入统一管理；CA 的"CRL"也会生成文件可导入此处留存。</div>
            </div>
          </n-tab-pane>
          <n-tab-pane name="pfx" tab="PFX">
            <div class="tb-sec">
              <div class="tb-h">
                打包 PFX（私钥 + 证书链 → .pfx）
                <n-tooltip trigger="hover"><template #trigger><Icon :icon="helpIcon" width="13" height="13" class="tb-help" /></template>
                把<b>一张私钥</b>与<b>至少一张证书</b>打包成 PKCS#12。证书按"叶证书在前、各级 CA 在后"的顺序填写。
                </n-tooltip>
              </div>
              <n-input v-model:value="pfxKey" type="textarea" size="small" :rows="3" placeholder="私钥 PEM（-----BEGIN PRIVATE KEY-----）" />
              <n-input v-model:value="pfxCerts" type="textarea" size="small" :rows="4" placeholder="证书链 PEM（叶证书在前，可含多张 CA）" />
              <div class="tb-row" style="margin-top:8px">
                <n-tooltip trigger="hover"><template #trigger><n-input v-model:value="pfxBuildPwd" type="password" size="small" placeholder="设置保护密码" show-password-on="click" style="width:200px" /></template>给生成的 PFX 设一个保护密码（导入方需要它）</n-tooltip>
                <n-tooltip trigger="hover"><template #trigger>
                  <button class="tb-ib primary lg" :disabled="pfxBuildLoading" @click="buildPfx"><Icon :icon="pfxBuildLoading ? refreshIcon : saveIcon" width="17" height="17" :class="{ 'tb-spin': pfxBuildLoading }" /></button>
                </template>打包并保存为 .pfx 文件</n-tooltip>
              </div>
            </div>

            <div class="tb-sec">
              <div class="tb-h">
                解包外部 PFX / P12
                <n-tooltip trigger="hover"><template #trigger><Icon :icon="helpIcon" width="13" height="13" class="tb-help" /></template>
                选一个外部 .pfx/.p12 文件，用密码拆出其中的全部私钥与证书链，可逐个查看、复制或保存。密码不对会报错，多试几个。
                </n-tooltip>
              </div>
              <div class="tb-row">
                <n-tooltip trigger="hover"><template #trigger><n-input v-model:value="pfxUnpackPwd" type="password" size="small" placeholder="PFX 保护密码（没有则留空）" show-password-on="click" style="width:240px" /></template>该 PFX 文件的保护密码；未加密的包留空即可</n-tooltip>
                <n-tooltip trigger="hover"><template #trigger>
                  <button class="tb-ib primary lg" :disabled="pfxUnpacking" @click="unpackPfxFile"><Icon :icon="pfxUnpacking ? refreshIcon : folderIcon" width="17" height="17" :class="{ 'tb-spin': pfxUnpacking }" /></button>
                </template>选择 .pfx/.p12 文件并解包</n-tooltip>
                <span class="tb-res" v-if="pfxName">{{ pfxName }}</span>
                <span v-if="error" class="tb-err">{{ error }}</span>
              </div>
              <template v-if="pfxKeys.length || pfxCertsOut.length">
                <div class="tb-sub">私钥（{{ pfxKeys.length }}）</div>
                <table class="tb-table" v-for="(k, i) in pfxKeys" :key="'k' + i">
                  <tbody><tr>
                    <td class="tb-res">私钥 #{{ i + 1 }}</td>
                    <td class="tb-ops">
                      <n-tooltip trigger="hover"><template #trigger><button class="tb-ib" @click="openView(`PFX 私钥 #${i+1}`, k)"><Icon :icon="eyeIcon" width="14" height="14" /></button></template>查看该私钥详情</n-tooltip>
                      <n-tooltip trigger="hover"><template #trigger><button class="tb-ib" @click="exportPem(k, pfxName + `.key${i+1}`)"><Icon :icon="saveIcon" width="14" height="14" /></button></template>保存该私钥为文件</n-tooltip>
                    </td>
                  </tr></tbody>
                </table>
                <div class="tb-sub">证书链（{{ pfxCertsOut.length }}）</div>
                <table class="tb-table" v-for="(c, i) in pfxCertsOut" :key="'c' + i">
                  <tbody><tr>
                    <td class="tb-res">{{ i === 0 ? '叶证书' : 'CA 证书' }} #{{ i + 1 }}</td>
                    <td class="tb-ops">
                      <n-tooltip trigger="hover"><template #trigger><button class="tb-ib" @click="openView(`PFX 证书 #${i+1}`, c)"><Icon :icon="eyeIcon" width="14" height="14" /></button></template>查看该证书详情</n-tooltip>
                      <n-tooltip trigger="hover"><template #trigger><button class="tb-ib" @click="exportPem(c, pfxName + `.crt${i+1}`)"><Icon :icon="saveIcon" width="14" height="14" /></button></template>保存该证书为文件</n-tooltip>
                    </td>
                  </tr></tbody>
                </table>
                <div class="tb-row">
                  <n-tooltip trigger="hover"><template #trigger>
                    <button class="tb-ib" @click="sendToEditor('```\n' + pfxKeys.join('\n') + pfxCertsOut.join('\n') + '\n```')"><Icon :icon="uploadIcon" width="15" height="15" /></button>
                  </template>把全部私钥与证书发送到主界面编辑器</n-tooltip>
                </div>
              </template>
            </div>
          </n-tab-pane>

          <n-tab-pane name="crypt" tab="加解密">
            <n-tabs type="segment" size="small" v-model:value="cryptTab" style="margin-top:4px">
              <n-tab-pane name="sym" tab="对称加解密">
                <div class="tb-sec">
                  <div class="tb-row">
                    <n-tooltip trigger="hover"><template #trigger><n-select v-model:value="symAlgo" size="small" :options="symAlgoOptions" style="width:260px" /></template>算法。分组密码支持 ECB/CBC/CFB/OFB/CTR；流密码与 AEAD 无需选模式</n-tooltip>
                    <n-tooltip v-if="symIsBlock" trigger="hover"><template #trigger><n-select v-model:value="symMode" size="small" :options="symModeOptions" style="width:230px" /></template>工作模式。CBC/CFB/OFB/CTR 需要 IV；ECB 不需要 IV 但更弱；CBC/ECB 可用 PKCS7 填充</n-tooltip>
                    <n-tooltip v-if="symIsBlock && (symMode === 'ecb' || symMode === 'cbc')" trigger="hover"><template #trigger><n-checkbox v-model:checked="symPad" size="small">PKCS7 填充</n-checkbox></template>勾选 = 自动做 PKCS7 填充（数据长度可任意）；不勾 = 要求输入是分组大小的整数倍</n-tooltip>
                  </div>
                  <div class="tb-row">
                    <n-tooltip trigger="hover"><template #trigger><n-input v-model:value="symKey" size="small" :placeholder="`key(hex, ${symKeyLen()} 字节)`" style="width:240px" /></template>密钥（十六进制），长度见占位符：{{ symEntry.label }} 需要 {{ symKeyLen() }} 字节</n-tooltip>
                    <n-tooltip trigger="hover"><template #trigger><button class="tb-ib" @click="genSymKey"><Icon :icon="wandIcon" width="15" height="15" /></button></template>用安全随机生成密钥（{{ symKeyLen() }} 字节）</n-tooltip>
                    <template v-if="symNeedsIv || !symIsBlock">
                      <n-tooltip trigger="hover"><template #trigger><n-input v-model:value="symIv" size="small" :placeholder="`${symIsAead ? 'nonce' : 'iv'}(hex, ${symIvLen()} 字节)`" style="width:210px" /></template>{{ symIsAead ? 'nonce' : 'IV' }}（十六进制），需要 {{ symIvLen() }} 字节</n-tooltip>
                      <n-tooltip trigger="hover"><template #trigger><button class="tb-ib" @click="genSymIv"><Icon :icon="refreshIcon" width="15" height="15" /></button></template>用安全随机生成 {{ symIsAead ? 'nonce' : 'IV' }}</n-tooltip>
                    </template>
                    <span v-else class="tb-hint">ECB 模式不需要 IV</span>
                  </div>
                  <div class="tb-hint">
                    当前：{{ symEntry.label }}<template v-if="symIsBlock"> · 模式 {{ symMode.toUpperCase() }} · 分组 {{ symEntry.block }} 字节</template>
                    <template v-if="symIsAead"> · AEAD，密文末尾附 16 字节认证标签，解密时密钥/nonce/AAD 必须与加密一致</template>
                    <template v-else-if="!symIsBlock"> · 流密码，密钥长度须 {{ symKeyLen() }} 字节</template>
                    <template v-else-if="symMode==='ecb' || symMode==='cbc'"> · 可勾选 PKCS7 填充处理任意长度数据</template>
                    <template v-else> · 该模式要求输入长度为 {{ symEntry.block }} 字节的整数倍</template>
                  </div>
                  <n-input v-model:value="symData" type="textarea" size="small" :rows="3" placeholder="数据（加密时勾选「明文输入」则填文本，否则填 base64；解密填 base64 密文）" />
                  <div class="tb-row" style="margin-top:8px">
                    <n-tooltip v-if="symIsAead" trigger="hover"><template #trigger><n-input v-model:value="symAad" size="small" placeholder="AAD(hex, 可空)" style="width:200px" /></template>附加认证数据（仅 AEAD），十六进制，可留空</n-tooltip>
                    <n-tooltip trigger="hover"><template #trigger><n-checkbox v-model:checked="symTextInput" size="small">明文输入</n-checkbox></template>勾选 = 输入是纯文本（不会按 base64 解码）；仅加密时生效</n-tooltip>
                    <n-tooltip trigger="hover"><template #trigger><button class="tb-ib primary lg" @click="runSym(true)"><Icon :icon="keyIcon" width="16" height="16" /></button></template>加密</n-tooltip>
                    <n-tooltip trigger="hover"><template #trigger><button class="tb-ib lg" @click="runSym(false)"><Icon :icon="eyeIcon" width="16" height="16" /></button></template>解密</n-tooltip>
                    <span v-if="error" class="tb-err">{{ error }}</span>
                  </div>
                  <pre class="tb-out" v-if="symOut">{{ symOut }}</pre>
                </div>
              </n-tab-pane>

              <n-tab-pane name="asym" tab="非对称(RSA)">
                <div class="tb-sec">
                  <div class="tb-row">
                    <n-tooltip trigger="hover"><template #trigger><n-select v-model:value="rsaScheme" size="small" :options="rsaSchemeOptions" style="width:190px" /></template>填充方案。OAEP + SHA-256 更安全（推荐）；PKCS#1 v1.5 兼容老系统</n-tooltip>
                    <n-tooltip trigger="hover"><template #trigger><n-checkbox v-model:checked="rsaTextInput" size="small">明文输入</n-checkbox></template>勾选 = 待加密内容是纯文本</n-tooltip>
                    <n-tooltip trigger="hover"><template #trigger><button class="tb-ib primary lg" @click="runRsa('encrypt')"><Icon :icon="keyIcon" width="16" height="16" /></button></template>用公钥加密</n-tooltip>
                    <n-tooltip trigger="hover"><template #trigger><button class="tb-ib lg" @click="runRsa('decrypt')"><Icon :icon="eyeIcon" width="16" height="16" /></button></template>用私钥解密</n-tooltip>
                  </div>
                  <div class="tb-hint">公钥加密、私钥解密；RSA 单次加密明文长度受密钥位数限制（OAEP/SHA-256 下 2048 位约 190 字节）。大数据请用对称密钥 + RSA 封装。</div>
                  <n-input v-model:value="rsaKey" type="textarea" size="small" :rows="3" placeholder="公钥 PEM(加密) / 私钥 PEM(解密)" />
                  <n-input v-model:value="rsaData" type="textarea" size="small" :rows="2" placeholder="数据" />
                  <pre class="tb-out" v-if="rsaOut">{{ rsaOut }}</pre>
                </div>
              </n-tab-pane>

              <n-tab-pane name="hash" tab="散列">
                <div class="tb-sec">
                  <div class="tb-row">
                    <n-tooltip trigger="hover"><template #trigger><n-select v-model:value="hashAlgo" size="small" :options="hashAlgoOptions" style="width:150px" /></template>摘要算法。SHA-256 为通用推荐；MD5/SHA-1 已不安全，仅用于校验兼容</n-tooltip>
                    <n-tooltip trigger="hover"><template #trigger><n-checkbox v-model:checked="hashTextInput" size="small">明文输入</n-checkbox></template>勾选 = 输入是纯文本；不勾 = 按 base64 解码后再算摘要</n-tooltip>
                    <n-tooltip trigger="hover"><template #trigger><button class="tb-ib primary lg" @click="runHash"><Icon :icon="playIcon" width="16" height="16" /></button></template>计算摘要</n-tooltip>
                  </div>
                  <n-input v-model:value="hashData" type="textarea" size="small" :rows="2" placeholder="数据" />
                  <pre class="tb-out" v-if="hashOut">{{ hashOut }}</pre>
                </div>
              </n-tab-pane>
            </n-tabs>
          </n-tab-pane>
        </n-tabs>
      </n-tab-pane>

      <n-tab-pane name="bin" tab="二进制分析">
        <template #tab><span class="tw-tab"><Icon :icon="binaryIcon" width="15" height="15" />二进制分析</span></template>
        <div class="tb-sec">
          <div class="tb-row">
            <n-button size="small" type="primary" :loading="binLoading" @click="pickBinary">选择 ELF/PE 文件</n-button>
            <span class="tb-res" v-if="binReport">{{ binReport.format }} · {{ binReport.arch }} · 总熵 {{ binReport.overall_entropy.toFixed(2) }}</span>
            <span v-if="error" class="tb-err">{{ error }}</span>
          </div>
          <table class="tb-table" v-if="binReport && binReport.sections.length">
            <thead><tr><th>段</th><th>大小</th><th>虚拟地址</th><th>熵(0-8)</th></tr></thead>
            <tbody>
              <tr v-for="s in binReport.sections" :key="s.name + s.vaddr">
                <td>{{ s.name }}</td>
                <td>{{ s.size.toLocaleString() }}</td>
                <td>{{ s.vaddr }}</td>
                <td>
                  <div class="tb-ent"><div class="tb-ent-bar" :style="{ width: (s.entropy/8*100)+'%', background: s.entropy>7?'#e5484d':(s.entropy>5?'#f5a524':'#30a46c') }"></div><span>{{ s.entropy.toFixed(2) }}</span></div>
                </td>
              </tr>
            </tbody>
          </table>
          <div class="tb-hint">熵 &gt;7 多为加密/压缩段，&lt;5 多为代码/数据。</div>
        </div>
      </n-tab-pane>

      <n-tab-pane name="core" tab="Coredump">
        <template #tab><span class="tw-tab"><Icon :icon="skullIcon" width="15" height="15" />Coredump</span></template>
        <div class="tb-sec tb-sec-fill">
          <div class="tb-row">
            <n-select v-model:value="profileId" size="small" :options="profileOptions" placeholder="SSH 档案" style="width:180px" />
            <n-input v-model:value="coreProgram" size="small" placeholder="可执行文件路径" style="width:200px" />
            <n-input v-model:value="coreFile" size="small" placeholder="core 文件路径" style="width:200px" />
            <n-button size="small" type="primary" :loading="coreLoading" @click="analyzeCore">分析</n-button>
            <n-button size="small" :disabled="!coreOut" @click="sendToEditor(coreOut)">→ 编辑器</n-button>
            <div class="tb-fontctl" title="输出字号">
              <button class="tb-fbtn" @click="setCoreFontSize(coreFontSize - 1)"><Icon :icon="minusIcon" width="13" height="13" /></button>
              <span class="tb-fsz">{{ coreFontSize }}</span>
              <button class="tb-fbtn" @click="setCoreFontSize(coreFontSize + 1)"><Icon :icon="plusIcon" width="13" height="13" /></button>
            </div>
          </div>
          <pre class="tb-out tb-out-fill" :style="{ fontSize: coreFontSize + 'px' }">{{ coreOut }}</pre>
        </div>
      </n-tab-pane>

      <n-tab-pane name="exec" tab="远程执行">
        <template #tab><span class="tw-tab"><Icon :icon="zapIcon" width="15" height="15" />远程执行</span></template>
        <div class="tb-sec">
          <div class="tb-row">
            <n-select v-model:value="profileId" size="small" :options="profileOptions" placeholder="SSH 档案" style="width:180px" />
            <n-input v-model:value="execCmd" size="small" placeholder="远程命令，如 dmesg | tail -100" style="flex:1;min-width:240px" @keyup.enter="runExec" />
            <n-button size="small" type="primary" :loading="execLoading" @click="runExec">执行</n-button>
            <n-button size="small" :disabled="!execSaved" @click="importCapture">导入分析</n-button>
          </div>
          <div class="tb-row">
            <span class="tb-h">定时采集</span>
            <n-input-number v-model:value="periodSecs" size="small" :min="1" style="width:110px" />
            <n-button size="small" v-if="!collectorId" @click="startPeriodic">开始</n-button>
            <n-button size="small" v-else @click="stopPeriodic">停止采集</n-button>
            <span class="tb-res" v-if="collectorId">采集中 → {{ collectorPath }}</span>
          </div>
          <pre class="tb-out">{{ execOut }}</pre>
        </div>
      </n-tab-pane>
      <n-tab-pane name="multi" tab="多机对齐">
        <template #tab><span class="tw-tab"><Icon :icon="gitCompareIcon" width="15" height="15" />多机对齐</span></template>
        <div class="tb-sec">
          <div class="tb-row">
            <n-button size="small" type="primary" @click="addMultiLog">添加日志</n-button>
            <n-button size="small" :disabled="!multiLogs.length" @click="clearMultiLogs">清空</n-button>
            <span class="tb-res">{{ multiLogs.length }} 台机器/日志，按时间戳对齐到同一时间线</span>
          </div>
          <VChart v-if="multiChartOption" class="tb-multi-chart" :option="multiChartOption" autoresize />
          <div v-else class="tb-hint">导入多份日志（如 client 与 server），每份的快照会排到同一条时间轴上，方便对因果。</div>
        </div>
      </n-tab-pane>

      <n-tab-pane name="dispatch" tab="命令下发" class="cd-pane">
        <template #tab><span class="tw-tab"><Icon :icon="sendIcon" width="15" height="15" />命令下发</span></template>
        <CommandDispatchPanel />
      </n-tab-pane>
    </n-tabs>

    <!-- 查看弹窗：字段明细 / PEM / DER(base64) / ASN.1，均可一键复制 -->
    <n-modal v-model:show="viewOpen" :mask-closable="true" style="width:820px;max-width:94vw">
      <div class="tv-modal">
        <div class="tv-head">
          <Icon :icon="certificateIcon" width="16" height="16" class="tv-head-ico" />
          <span class="tv-title">{{ viewTitle }}</span>
          <n-tooltip trigger="hover"><template #trigger>
            <button class="tb-ib" @click="viewOpen = false"><Icon :icon="xIcon" width="16" height="16" /></button>
          </template>关闭</n-tooltip>
        </div>
        <div class="tv-tabs">
          <button class="tv-tab" :class="{ active: viewTab === 'fields' }" @click="viewTab = 'fields'">字段明细</button>
          <button class="tv-tab" :class="{ active: viewTab === 'pem' }" @click="viewTab = 'pem'">PEM 原文</button>
          <button class="tv-tab" :class="{ active: viewTab === 'b64' }" @click="viewTab = 'b64'">DER (base64)</button>
          <button class="tv-tab" :class="{ active: viewTab === 'asn1' }" @click="viewTab = 'asn1'">ASN.1</button>
        </div>
        <div class="tv-body">
          <div v-if="viewLoading" class="tv-hint">解析中…</div>
          <div v-else-if="viewError" class="tv-err">{{ viewError }}</div>
          <template v-else-if="viewDetails">
            <template v-if="viewTab === 'fields'">
              <table class="tb-kv tv-kv">
                <tbody><tr v-for="(kv, i) in viewDetails.fields" :key="i"><td>{{ kv[0] }}</td><td class="tv-v">{{ kv[1] }}</td></tr></tbody>
              </table>
              <template v-if="viewDetails.sans.length">
                <div class="tv-sub">主题备用名 (SAN)</div>
                <div class="tv-sans">
                  <span v-for="(s, i) in viewDetails.sans" :key="i" class="tv-san">{{ s }}</span>
                </div>
              </template>
              <div class="tv-sub">SHA-256 指纹</div>
              <div class="tv-mono tv-fp">{{ viewDetails.fingerprint }}
                <n-tooltip trigger="hover"><template #trigger><button class="tb-ib" @click="doCopy(viewDetails.fingerprint,'fp')"><Icon :icon="copyIcon" width="14" height="14" /></button></template>复制指纹</n-tooltip>
              </div>
            </template>
            <template v-else-if="viewTab === 'pem'">
              <div class="tv-bar">
                <span class="tv-hint">标准 PEM 文本，可直接保存为 .pem/.crt/.key</span>
                <n-tooltip trigger="hover"><template #trigger><button class="tb-ib" @click="doCopy(viewDetails.pem,'pem')"><Icon :icon="copyIcon" width="15" height="15" /></button></template>复制 PEM</n-tooltip>
                <n-tooltip trigger="hover"><template #trigger><button class="tb-ib" @click="saveText(viewDetails.pem, safeFileName(viewTitle) + '.pem')"><Icon :icon="saveIcon" width="15" height="15" /></button></template>保存为文件</n-tooltip>
                <span v-if="viewCopied==='pem'" class="tv-ok">已复制</span>
              </div>
              <pre class="tv-pre">{{ viewDetails.pem }}</pre>
            </template>
            <template v-else-if="viewTab === 'b64'">
              <div class="tv-bar">
                <span class="tv-hint">DER 的 base64（不含 PEM 头尾），便于贴进程序或脚本</span>
                <n-tooltip trigger="hover"><template #trigger><button class="tb-ib" @click="doCopy(viewDetails.der_b64,'b64')"><Icon :icon="copyIcon" width="15" height="15" /></button></template>复制 base64</n-tooltip>
                <span v-if="viewCopied==='b64'" class="tv-ok">已复制</span>
              </div>
              <pre class="tv-pre">{{ viewDetails.der_b64 }}</pre>
            </template>
            <template v-else>
              <div class="tv-bar">
                <n-tooltip trigger="hover"><template #trigger><button class="tb-ib" @click="doCopy(viewDetails.pem,'pem')"><Icon :icon="copyIcon" width="15" height="15" /></button></template>复制 PEM（ASN.1 视图暂不支持直接复制结构树）</n-tooltip>
              </div>
              <div class="tv-hint">ASN.1 结构可在"解析/导入"页解析后查看下方结构树。</div>
            </template>
          </template>
        </div>
      </div>
    </n-modal>
  </div>
</template>

<script setup>
import { ref, computed, onMounted } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { open, save } from '@tauri-apps/plugin-dialog'
import { readFile } from '@tauri-apps/plugin-fs'
import { use } from 'echarts/core'
import { CanvasRenderer } from 'echarts/renderers'
import { ScatterChart } from 'echarts/charts'
import { GridComponent, TooltipComponent, LegendComponent, DataZoomComponent } from 'echarts/components'
import VChart from 'vue-echarts'
import { createLogger } from '../../../utils/logger'
import { Icon } from '@iconify/vue'
import calcIcon from '@iconify-icons/lucide/calculator'
import binaryIcon from '@iconify-icons/lucide/binary'
import skullIcon from '@iconify-icons/lucide/skull'
import zapIcon from '@iconify-icons/lucide/zap'
import gitCompareIcon from '@iconify-icons/lucide/git-compare'
import keyIcon from '@iconify-icons/lucide/key-round'
import minusIcon from '@iconify-icons/lucide/minus'
import plusIcon from '@iconify-icons/lucide/plus'
import eyeIcon from '@iconify-icons/lucide/eye'
import copyIcon from '@iconify-icons/lucide/copy'
import xIcon from '@iconify-icons/lucide/x'
import trashIcon from '@iconify-icons/lucide/trash-2'
import refreshIcon from '@iconify-icons/lucide/refresh-cw'
import downloadIcon from '@iconify-icons/lucide/download'
import uploadIcon from '@iconify-icons/lucide/upload'
import saveIcon from '@iconify-icons/lucide/save'
import folderIcon from '@iconify-icons/lucide/folder-open'
import fileIcon from '@iconify-icons/lucide/file-key'
import certificateIcon from '@iconify-icons/lucide/scroll-text'
import banIcon from '@iconify-icons/lucide/ban'
import listIcon from '@iconify-icons/lucide/list'
import playIcon from '@iconify-icons/lucide/play'
import wandIcon from '@iconify-icons/lucide/wand-2'
import helpIcon from '@iconify-icons/lucide/help-circle'
import sendIcon from '@iconify-icons/lucide/send'
import CommandDispatchPanel from './CommandDispatchPanel.vue'
import { writeText } from '@tauri-apps/plugin-clipboard-manager'

use([CanvasRenderer, ScatterChart, GridComponent, TooltipComponent, LegendComponent, DataZoomComponent])

const emit = defineEmits(['import-file'])
const logger = createLogger('RemoteToolbox')
const error = ref('')

// __TB_BODY__
const profiles = ref([])
const profileOptions = computed(() => profiles.value.map(p => ({ label: `${p.name || p.host}`, value: p.id })))
const profileId = ref(null)

const numInput = ref('')
const numOut = ref(null)
function bigintToAscii(v) {
  if (v < 0n) return '—'
  let bytes = []; let x = v
  while (x > 0n) { bytes.unshift(Number(x & 0xffn)); x >>= 8n }
  if (!bytes.length) bytes = [0]
  return bytes.map(b => (b >= 32 && b < 127) ? String.fromCharCode(b) : '.').join('')
}
function convertNum() {
  const s = numInput.value.trim()
  if (!s) { numOut.value = null; return }
  let v
  try {
    if (/^0x[0-9a-f]+$/i.test(s) || /^0b[01]+$/i.test(s) || /^0o[0-7]+$/i.test(s) || /^-?\d+$/.test(s)) v = BigInt(s)
    else { numOut.value = null; return }
  } catch (_) { numOut.value = null; return }
  numOut.value = {
    hex: '0x' + (v < 0n ? '-' + (-v).toString(16) : v.toString(16)),
    dec: v.toString(10),
    oct: '0o' + (v < 0n ? '-' + (-v).toString(8) : v.toString(8)),
    bin: '0b' + (v < 0n ? '-' + (-v).toString(2) : v.toString(2)),
    ascii: bigintToAscii(v),
  }
}

const opA = ref(''); const opB = ref(''); const op = ref('&'); const opResult = ref('')
const opOptions = [
  { label: 'AND', value: '&' }, { label: 'OR', value: '|' }, { label: 'XOR', value: '^' },
  { label: '<<', value: '<<' }, { label: '>>', value: '>>' },
  { label: '+', value: '+' }, { label: '-', value: '-' }, { label: '*', value: '*' },
]
function pnum(s) { s = (s || '').trim(); try { return BigInt(s) } catch (_) { return null } }
function calcOp() {
  const a = pnum(opA.value), b = pnum(opB.value)
  if (a == null || b == null) { opResult.value = '输入无效'; return }
  let r
  switch (op.value) {
    case '&': r = a & b; break; case '|': r = a | b; break; case '^': r = a ^ b; break
    case '<<': r = a << b; break; case '>>': r = a >> b; break
    case '+': r = a + b; break; case '-': r = a - b; break; case '*': r = a * b; break; default: r = 0n
  }
  const hx = r < 0n ? '-0x' + (-r).toString(16) : '0x' + r.toString(16)
  opResult.value = `${r.toString(10)}  =  ${hx}`
}

const epochInput = ref(''); const epochOut = ref(''); const dateInput = ref(''); const dateOut = ref('')
function epochToDate() {
  const s = epochInput.value.trim(); if (!s) { epochOut.value = ''; return }
  const n = Number(s); if (!isFinite(n)) { epochOut.value = '无效'; return }
  const ms = s.replace('-', '').length > 10 ? n : n * 1000
  const d = new Date(ms); epochOut.value = isNaN(d.getTime()) ? '无效' : d.toLocaleString() + '  /  ' + d.toISOString()
}
function dateToEpoch() {
  const s = dateInput.value.trim(); if (!s) { dateOut.value = ''; return }
  const d = new Date(s.replace(' ', 'T')); if (isNaN(d.getTime())) { dateOut.value = '无效'; return }
  dateOut.value = `epoch ${Math.floor(d.getTime() / 1000)} s  /  ${d.getTime()} ms`
}

// IP 转换（前端纯 JS）：点分/整数/十六进制互转 + CIDR 网段计算
const ipInput = ref(''); const ipOut = ref(null)
function intToIpv4(n) { return [(n >>> 24) & 255, (n >>> 16) & 255, (n >>> 8) & 255, n & 255].join('.') }
function ipv4ToInt(s) {
  const p = s.split('.'); if (p.length !== 4) return null
  let n = 0
  for (const x of p) { const b = Number(x); if (!Number.isInteger(b) || b < 0 || b > 255) return null; n = (n * 256) + b }
  return n >>> 0
}

// ---- IPv6 ----
// 把 IPv6 文本解析成 8 组 16 位数字（BigInt 各位），失败返回 null。
// 支持 :: 压缩、内嵌 IPv4（如 ::ffff:192.168.1.1）、大小写混写。
function ipv6ToGroups(s) {
  let t = s.trim()
  if (!t.includes(':')) return null
  // 内嵌 IPv4 尾巴：a:b:...:1.2.3.4 → 转成两组 16 位十六进制
  const m = t.match(/^(.*:)(\d+\.\d+\.\d+\.\d+)$/)
  if (m) {
    const v4 = ipv4ToInt(m[2]); if (v4 == null) return null
    t = m[1] + ((v4 >>> 16) & 0xffff).toString(16) + ':' + (v4 & 0xffff).toString(16)
  }
  const dbl = t.split('::')
  if (dbl.length > 2) return null
  const parse = seg => seg === '' ? [] : seg.split(':').map(g => {
    if (!/^[0-9a-fA-F]{1,4}$/.test(g)) return null
    return parseInt(g, 16)
  })
  const left = parse(dbl[0]); const right = dbl.length === 2 ? parse(dbl[1]) : []
  if (left.includes(null) || right.includes(null)) return null
  if (dbl.length === 2) {
    const fill = 8 - left.length - right.length
    if (fill < 0) return null
    return [...left, ...Array(fill).fill(0), ...right]
  }
  return left.length === 8 ? left : null
}
function ipv6ToBig(groups) {
  let n = 0n
  for (const g of groups) n = (n << 16n) | BigInt(g)
  return n
}
function bigToIpv6Groups(n) {
  const g = []
  for (let i = 7; i >= 0; i--) g.push(Number((n >> BigInt(i * 16)) & 0xffffn))
  return g
}
// RFC 5952 规范压缩：小写、去前导零、最长零段用 :: 代替（≥2 段才压）。
function groupsToIpv6(groups) {
  let best = -1, bestLen = 0, cur = -1, curLen = 0
  for (let i = 0; i < 8; i++) {
    if (groups[i] === 0) { if (cur < 0) { cur = i; curLen = 1 } else curLen++ } else { cur = -1; curLen = 0 }
    if (cur >= 0 && curLen > bestLen) { best = cur; bestLen = curLen }
  }
  if (bestLen < 2) return groups.map(g => g.toString(16)).join(':')
  const head = groups.slice(0, best).map(g => g.toString(16)).join(':')
  const tail = groups.slice(best + bestLen).map(g => g.toString(16)).join(':')
  return head + '::' + tail
}
function groupsToExpanded(groups) {
  return groups.map(g => g.toString(16).padStart(4, '0')).join(':')
}
function groupsToBinary(groups) {
  return groups.map(g => g.toString(2).padStart(16, '0')).join(':')
}
// IPv6 常见归类，便于快速判断地址用途。
function ipv6Kind(groups) {
  const first = groups[0]
  if (groups.every(g => g === 0)) return '未指定地址 (::)'
  if (groups.slice(0, 7).every(g => g === 0) && groups[7] === 1) return '环回 (::1)'
  if (first === 0x2002) return '6to4'
  if (first === 0x2001 && groups[1] === 0x0db8) return '文档示例 (2001:db8::/32)'
  if (first === 0x2001 && groups[1] === 0x0000) return 'Teredo'
  if (first === 0xfe80) return '链路本地 (fe80::/10)'
  if ((first & 0xffc0) === 0xfec0) return '站点本地 (fec0::/10, 已废弃)'
  if ((first & 0xfe00) === 0xfc00) return '唯一本地 ULA (fc00::/7)'
  if ((first & 0xff00) === 0xff00) return '组播 (ff00::/8)'
  if ((first & 0xe000) === 0x2000) return '全球单播 (2000::/3)'
  return '保留/其他'
}

function convertIp() {
  const s = ipInput.value.trim()
  if (!s) { ipOut.value = null; return }

  // ================= IPv6 =================
  if (s.includes(':')) {
    const [addrPart, bitsStr] = s.includes('/') ? s.split('/') : [s, null]
    const g = ipv6ToGroups(addrPart.trim())
    if (!g) { ipOut.value = { 错误: '无效 IPv6 地址' }; return }
    if (bitsStr != null) {
      const bits = Number(bitsStr)
      if (!Number.isInteger(bits) || bits < 0 || bits > 128) { ipOut.value = { 错误: '无效 IPv6 CIDR（前缀 0–128）' }; return }
      const mask = bits === 0 ? 0n : (((1n << BigInt(bits)) - 1n) << BigInt(128 - bits))
      const full = (1n << 128n) - 1n
      const n = ipv6ToBig(g)
      const network = n & mask
      const last = network | (full ^ mask)
      const hostCount = bits >= 127 ? (bits === 128 ? 1n : 2n) : (last - network - 1n)
      const firstHost = bits >= 127 ? network : network + 1n
      const lastHost = bits >= 127 ? last : last - 1n
      ipOut.value = {
        '地址类型': ipv6Kind(g),
        '网段 (压缩)': `${groupsToIpv6(bigToIpv6Groups(network))}/${bits}`,
        '网段 (展开)': `${groupsToExpanded(bigToIpv6Groups(network))}/${bits}`,
        '前缀掩码': groupsToIpv6(bigToIpv6Groups(mask)),
        '网络地址': groupsToIpv6(bigToIpv6Groups(network)),
        '末地址': groupsToIpv6(bigToIpv6Groups(last)),
        '可用范围': bits >= 128 ? '(单地址)' : `${groupsToIpv6(bigToIpv6Groups(firstHost))} - ${groupsToIpv6(bigToIpv6Groups(lastHost))}`,
        '地址总数': (2n ** BigInt(128 - bits)).toLocaleString('en-US'),
        '可用主机数': hostCount.toLocaleString('en-US'),
      }
      return
    }
    const n = ipv6ToBig(g)
    ipOut.value = {
      '地址类型': ipv6Kind(g),
      '压缩写法': groupsToIpv6(g),
      '完整展开': groupsToExpanded(g),
      '十六进制': '0x' + n.toString(16).padStart(32, '0'),
      '十进制 (大整数)': n.toString(),
      '二进制': groupsToBinary(g),
      '内嵌 IPv4 (如适用)': (g[0] === 0 && g[1] === 0 && g[2] === 0 && g[3] === 0 && g[4] === 0 && g[5] === 0xffff)
        ? intToIpv4(((g[6] << 16) | g[7]) >>> 0) : '—',
    }
    return
  }

  // ================= IPv4 =================
  if (s.includes('/')) {
    const [ip, bitsStr] = s.split('/')
    const base = ipv4ToInt(ip.trim()); const bits = Number(bitsStr)
    if (base == null || !Number.isInteger(bits) || bits < 0 || bits > 32) { ipOut.value = { 错误: '无效 CIDR' }; return }
    const mask = bits === 0 ? 0 : (0xffffffff << (32 - bits)) >>> 0
    const network = (base & mask) >>> 0
    const broadcast = (network | (~mask >>> 0)) >>> 0
    const hosts = bits >= 31 ? (bits === 32 ? 1 : 2) : ((broadcast - network - 1) >>> 0)
    ipOut.value = {
      网段: `${intToIpv4(network)}/${bits}`,
      子网掩码: intToIpv4(mask),
      网络地址: intToIpv4(network),
      广播地址: intToIpv4(broadcast),
      可用范围: `${intToIpv4((network + (bits >= 31 ? 0 : 1)) >>> 0)} - ${intToIpv4((broadcast - (bits >= 31 ? 0 : 1)) >>> 0)}`,
      主机数: hosts.toLocaleString(),
    }
    return
  }
  let n = null
  if (/^\d+\.\d+\.\d+\.\d+$/.test(s)) n = ipv4ToInt(s)
  else if (/^0x[0-9a-f]+$/i.test(s)) n = parseInt(s, 16) >>> 0
  else if (/^\d+$/.test(s)) n = Number(s) >>> 0
  if (n == null || isNaN(n)) { ipOut.value = { 错误: '无法识别（支持 IPv4 点分/十进制/0x十六进制/CIDR，或 IPv6 地址/CIDR）' }; return }
  // IPv4 也给出对应 IPv4-mapped IPv6 写法，方便对照
  const mapped = [0, 0, 0, 0, 0, 0xffff, (n >>> 16) & 0xffff, n & 0xffff]
  ipOut.value = {
    点分十进制: intToIpv4(n),
    十进制: (n >>> 0).toString(),
    十六进制: '0x' + (n >>> 0).toString(16).toUpperCase().padStart(8, '0'),
    二进制: (n >>> 0).toString(2).padStart(32, '0').replace(/(.{8})(?=.)/g, '$1.'),
    'IPv4-mapped IPv6': groupsToIpv6(mapped),
  }
}

// 证书 / ASN.1 解析（后端 x509-parser）
const cryptoInput = ref(''); const cryptoReport = ref(null); const cryptoLoading = ref(false)
const cryptoAsText = computed(() => {
  if (!cryptoReport.value) return ''
  const s = cryptoReport.value.summary.map(kv => `${kv[0]}: ${kv[1]}`).join('\n')
  return '```\n' + s + '\n\n' + cryptoReport.value.asn1 + '\n```\n'
})
async function runCrypto() {
  error.value = ''
  if (!cryptoInput.value.trim()) return
  cryptoLoading.value = true
  try { cryptoReport.value = await invoke('crypto_inspect', { input: cryptoInput.value }) }
  catch (e) { error.value = '解析失败: ' + (e?.message || e); cryptoReport.value = null }
  finally { cryptoLoading.value = false }
}
// __TB_BODY2__
// ============ 证书/密钥/加解密工作台（后端 crypto_lab）============
const cxTab = ref('parse')

// ============ 查看弹窗：字段明细 / PEM / DER(base64)，均可复制 ============
const viewOpen = ref(false)
const viewTitle = ref('')
const viewLoading = ref(false)
const viewTab = ref('fields')
const viewError = ref('')
const viewDetails = ref(null) // { kind, pem, der_b64, fingerprint, fields, sans }
const viewCopied = ref('')
async function openView(label, pem) {
  if (!pem || !String(pem).trim()) { error.value = '该项无内容可查看'; return }
  viewTitle.value = label
  viewOpen.value = true
  viewTab.value = 'fields'
  viewError.value = ''
  viewDetails.value = null
  viewLoading.value = true
  try {
    viewDetails.value = await invoke('crypto_pem_details', { input: String(pem) })
  } catch (e) {
    viewError.value = '解析失败: ' + (e?.message || e)
  } finally {
    viewLoading.value = false
  }
}
async function doCopy(text, tag) {
  try {
    await writeText(String(text))
    viewCopied.value = tag
    setTimeout(() => { if (viewCopied.value === tag) viewCopied.value = '' }, 1500)
  } catch (e) { error.value = '复制失败: ' + (e?.message || e) }
}
// 把标题变成安全的文件名（去掉文件系统非法字符）。
function safeFileName(s) {
  return String(s || 'item').replace(/[\\/:*?<>|·\s]+/g, '_').slice(0, 80)
}

// ============ 主题串解析：支持 openssl 的 /C=CN/ST=x/... 与逗号形式 ============
const subjectText = ref('')
const subjectHint = ref('')
function parseSubjectString() {
  const raw = subjectText.value.trim()
  subjectHint.value = ''
  if (!raw) { subjectHint.value = '请输入主题串'; return }
  // /C=CN/ST=Shannxi/L=xi'an/O=XA/OU=Dev/CN=Root CA  或  C=CN, ST=x, O=y, CN=z
  const map = {}
  const re = /\/([A-Za-z]+)\s*=\s*([^/]*)/g
  let m
  let matched = false
  while ((m = re.exec(raw)) !== null) {
    map[m[1].toUpperCase()] = m[2].trim()
    matched = true
  }
  if (!matched) {
    for (const part of raw.split(/[,\n]/)) {
      const i = part.indexOf('=')
      if (i > 0) {
        map[part.slice(0, i).trim().toUpperCase()] = part.slice(i + 1).trim()
        matched = true
      }
    }
  }
  if (!matched) { subjectHint.value = '无法识别，示例：/C=CN/ST=Shannxi/L=xi\'an/O=XA/OU=Dev/CN=Root CA'; return }
  const applied = []
  if (map.C != null) { subjC.value = map.C; applied.push('C') }
  if (map.ST != null) { subjST.value = map.ST; applied.push('ST') }
  if (map.L != null) { subjL.value = map.L; applied.push('L') }
  if (map.O != null) { subjO.value = map.O; applied.push('O') }
  if (map.OU != null) { subjOU.value = map.OU; applied.push('OU') }
  if (map.CN != null) { caCn.value = map.CN; leafCn.value = map.CN; applied.push('CN') }
  subjectHint.value = applied.length ? `已填入: ${applied.join(' / ')}` : '未识别到 C/ST/L/O/OU/CN 字段'
}
const loadedKind = ref(''); const pfxPassword = ref(''); const pfxOut = ref('')
function u8ToB64(u8) { let s = ''; const chunk = 0x8000; for (let i = 0; i < u8.length; i += chunk) s += String.fromCharCode.apply(null, u8.subarray(i, i + chunk)); return btoa(s) }
function b64ToU8(b) { const bin = atob(b); const u = new Uint8Array(bin.length); for (let i = 0; i < bin.length; i++) u[i] = bin.charCodeAt(i); return u }
function strToB64(s) { return u8ToB64(new TextEncoder().encode(s)) }
function b64ToText(b) { try { return new TextDecoder().decode(b64ToU8(b)) } catch (_) { return '(非文本)' } }
function splitPem(t) { return (t.match(/-----BEGIN[\s\S]*?-----END[^-]*-----/g) || []).map(s => s.trim()) }

async function pickCryptoFile() {
  error.value = ''; pfxOut.value = ''; loadedKind.value = ''
  const sel = await open({ multiple: false, directory: false, title: '选择证书/密钥/PFX 文件' })
  if (!sel) return
  const path = typeof sel === 'string' ? sel : sel?.path
  try {
    const bytes = await readFile(path)
    const name = path.replace(/\\/g, '/').split('/').pop()
    const loaded = await invoke('crypto_load_bytes', { dataB64: u8ToB64(bytes), fileName: name })
    loadedKind.value = loaded.kind
    if (loaded.kind === 'pkcs12') {
      const r = await invoke('crypto_pfx_parse', { dataB64: loaded.content, password: pfxPassword.value || '' })
      pfxOut.value = ['# 私钥', ...r.keys_pem, '# 证书链', ...r.certs_pem].join('\n')
      cryptoInput.value = r.certs_pem[0] || ''
    } else {
      cryptoInput.value = loaded.content
      await runCrypto()
    }
  } catch (e) { error.value = '读取失败: ' + (e?.message || e) }
}

const keyAlgo = ref('ec'); const rsaBits = ref(2048); const ecCurve = ref('p256')
// 算法清单：列出当前库（rcgen + ring / RustCrypto）实际支持的全部组合。
// 注意 P-521 需要 rcgen 的 aws_lc_rs 后端（本机无 NASM，故不提供）；SM2/PQC 需 tlcp 特性。
const keyAlgoOptions = [
  { label: 'RSA（推荐 ≥2048）', value: 'rsa' },
  { label: 'ECDSA（P-256/P-384）', value: 'ec' },
  { label: 'Ed25519（EdDSA）', value: 'ed25519' },
]
const rsaBitsOptions = [
  { label: 'RSA-1024（不安全，仅测试）', value: 1024 },
  { label: 'RSA-2048（默认）', value: 2048 },
  { label: 'RSA-3072', value: 3072 },
  { label: 'RSA-4096', value: 4096 },
  { label: 'RSA-8192（很慢）', value: 8192 },
]
const ecCurveOptions = [
  { label: 'P-256 (prime256v1，默认)', value: 'p256' },
  { label: 'P-384 (secp384r1)', value: 'p384' },
]
const genPrivPem = ref(''); const genPubPem = ref(''); const keyLoading = ref(false)
async function genKey() {
  error.value = ''; keyLoading.value = true
  try {
    const r = await invoke('crypto_gen_key', { algo: keyAlgo.value, rsaBits: keyAlgo.value === 'rsa' ? rsaBits.value : null, curve: keyAlgo.value === 'ec' ? ecCurve.value : null })
    genPrivPem.value = r.private_pem; genPubPem.value = r.public_pem
  } catch (e) { error.value = '生成失败: ' + (e?.message || e) } finally { keyLoading.value = false }
}

const csrCn = ref(''); const csrOrg = ref(''); const csrSans = ref(''); const csrKey = ref(''); const csrOut = ref(''); const csrLoading = ref(false)
async function genCsr() {
  error.value = ''
  const key = csrKey.value.trim() || genPrivPem.value
  if (!key) { error.value = '需要私钥（填入或先生成）'; return }
  csrLoading.value = true
  try {
    csrOut.value = await invoke('crypto_gen_csr', { keyPem: key, commonName: csrCn.value, org: csrOrg.value || null, sans: csrSans.value.split(',').map(s => s.trim()).filter(Boolean) })
  } catch (e) { error.value = 'CSR 生成失败: ' + (e?.message || e) } finally { csrLoading.value = false }
}

// ---- 证书链/签发：CA 库驱动（后台记录 CA，签发下拉选，无需粘贴 PEM）----
// 主题默认值（用户不填即用这些）；与 gencert.sh 里 openssl -subj 的写法一致。
const subjC = ref('CN'); const subjST = ref('Shannxi'); const subjL = ref("xi'an")
const subjO = ref('XA'); const subjOU = ref('Dev')
const chainBits = ref(2048); const chainCurve = ref('p256'); const chainDays = ref(3650)
const caCategory = ref('ecc')
const caCategoryOptions = [
  { label: 'RSA', value: 'rsa' }, { label: 'ECC', value: 'ecc' },
  { label: 'SM2 (国密)', value: 'sm2' }, { label: 'PQC (后量子)', value: 'pqc' },
]
const caList = ref([])
const leafList = ref([])
const caName = ref(''); const caCn = ref(''); const caParent = ref(null); const caLoading = ref(false)
const issueCaId = ref(null); const leafProfile = ref('server'); const leafCn = ref(''); const leafSans = ref('')
const leafLoading = ref(false); const leafCertOut = ref(''); const leafKeyOut = ref('')
const importCaName = ref(''); const importCertPem = ref(''); const importKeyPem = ref('')
const leafProfileOptions = [{ label: '服务端', value: 'server' }, { label: '客户端', value: 'client' }]
const catLabel = { rsa: 'RSA', ecc: 'ECC', sm2: 'SM2', pqc: 'PQC', imported: '导入' }
const caSelectOptions = computed(() => caList.value.map(c => ({ label: `[${catLabel[c.category] || c.category}] ${c.name} (${c.is_root ? '根' : '中间'})`, value: c.id })))
const caParentOptions = computed(() => caList.value.filter(c => c.category === caCategory.value).map(c => ({ label: c.name, value: c.id })))
// 四类 CA 库分组
const caGroups = computed(() => ['rsa', 'ecc', 'sm2', 'pqc'].map(cat => ({ cat, label: catLabel[cat], items: caList.value.filter(c => c.category === cat) })))
function subjectObj(cn) {
  return { country: subjC.value || null, state: subjST.value || null, locality: subjL.value || null, org: subjO.value || null, orgUnit: subjOU.value || null, commonName: cn }
}
async function loadCaList() { try { caList.value = await invoke('crypto_ca_list'); leafList.value = await invoke('crypto_leaf_list') } catch (e) { error.value = '读库失败: ' + (e?.message || e) } }
async function createCa() {
  error.value = ''
  if (!caName.value.trim() || !caCn.value.trim()) { error.value = '需要 CA 名称 + CN'; return }
  caLoading.value = true
  try {
    await invoke('crypto_ca_create', {
      name: caName.value, category: caCategory.value, subject: subjectObj(caCn.value),
      rsaBits: caCategory.value === 'rsa' ? chainBits.value : null,
      curve: caCategory.value === 'ecc' ? chainCurve.value : null,
      days: chainDays.value, issuerCaId: caParent.value || null,
    })
    caName.value = ''; caCn.value = ''; caParent.value = null
    await loadCaList()
  } catch (e) { error.value = '生成 CA 失败: ' + (e?.message || e) } finally { caLoading.value = false }
}
async function issueLeaf() {
  error.value = ''; leafCertOut.value = ''; leafKeyOut.value = ''
  if (!issueCaId.value || !leafCn.value.trim()) { error.value = '需要选择签发 CA + 填 CN'; return }
  leafLoading.value = true
  try {
    const r = await invoke('crypto_issue_from_ca', {
      caId: issueCaId.value, subject: subjectObj(leafCn.value), profile: leafProfile.value,
      sans: leafSans.value.split(',').map(s => s.trim()).filter(Boolean), days: chainDays.value,
    })
    leafCertOut.value = r.cert_pem; leafKeyOut.value = r.key_pem || ''
    await loadCaList()
  } catch (e) { error.value = '签发失败: ' + (e?.message || e) } finally { leafLoading.value = false }
}
async function revokeCa(ca_id, serial) {
  const reason = window.prompt('吊销原因 (keyCompromise/superseded/cessationOfOperation/…)', 'superseded')
  if (reason === null) return
  try { await invoke('crypto_ca_revoke', { caId: ca_id, serial, reason }); error.value = `已登记吊销 serial=${serial}` }
  catch (e) { error.value = '吊销失败: ' + (e?.message || e) }
}
async function genCrl(c) {
  try {
    const pem = await invoke('crypto_ca_gen_crl', { caId: c.id })
    await saveText(pem, (c.name || 'ca') + '.crl.pem')
    await loadCaList()
  } catch (e) { error.value = '生成 CRL 失败: ' + (e?.message || e) }
}
async function exportLeaf(l, which) {
  try {
    const r = await invoke('crypto_leaf_export', { id: l.id })
    if (which === 'cert') await saveText(r.cert_pem, l.name + '.crt')
    else if (which === 'key') await saveText(r.key_pem, l.name + '.key')
    else await saveText(r.chain_pem, l.name + '.chain.pem')
  } catch (e) { error.value = '导出失败: ' + (e?.message || e) }
}
// 查看：证书 / 私钥 / 证书链，打开详情弹窗（字段 + PEM + base64，可复制）。
async function viewLeaf(l, which) {
  try {
    const r = await invoke('crypto_leaf_export', { id: l.id })
    const label = which === 'key' ? `私钥 · ${l.name}` : which === 'chain' ? `证书链 · ${l.name}` : `证书 · ${l.name}`
    await openView(label, which === 'key' ? r.key_pem : which === 'chain' ? r.chain_pem : r.cert_pem)
  } catch (e) { error.value = '查看失败: ' + (e?.message || e) }
}
async function viewCa(c, which) {
  try {
    const r = await invoke('crypto_ca_export', { id: c.id })
    await openView(which === 'key' ? `CA 私钥 · ${c.name}` : `CA 证书 · ${c.name}`, which === 'key' ? r.key_pem : r.cert_pem)
  } catch (e) { error.value = '查看失败: ' + (e?.message || e) }
}
async function deleteLeaf(l) {
  if (!window.confirm(`从库中删除证书「${l.name}」？`)) return
  try { await invoke('crypto_leaf_delete', { id: l.id }); await loadCaList() } catch (e) { error.value = '删除失败: ' + (e?.message || e) }
}
async function exportCa(c) {
  try {
    const r = await invoke('crypto_ca_export', { id: c.id })
    const dir = await save({ title: '保存 CA（选基名，会写 .crt 与 .key）', defaultPath: (c.name || 'ca') + '.crt' })
    if (!dir) return
    const base = String(dir).replace(/\.(crt|cer|pem)$/i, '')
    await invoke('write_file_content', { path: base + '.crt', content: r.cert_pem })
    await invoke('write_file_content', { path: base + '.key', content: r.key_pem })
    error.value = '已导出: ' + base + '.crt / .key'
  } catch (e) { error.value = '导出失败: ' + (e?.message || e) }
}
async function deleteCa(c) {
  if (!window.confirm(`从库中删除 CA「${c.name}」？（不影响已签发的证书）`)) return
  try { await invoke('crypto_ca_delete', { id: c.id }); await loadCaList() } catch (e) { error.value = '删除失败: ' + (e?.message || e) }
}
async function pickCaFile(which) {
  const sel = await open({ multiple: false, directory: false, title: which === 'cert' ? '选择 CA 证书' : '选择 CA 私钥' })
  if (!sel) return
  const path = typeof sel === 'string' ? sel : sel?.path
  try {
    const txt = new TextDecoder().decode(await readFile(path))
    if (which === 'cert') importCertPem.value = txt; else importKeyPem.value = txt
  } catch (e) { error.value = '读取失败: ' + (e?.message || e) }
}
async function importCa() {
  error.value = ''
  if (!importCaName.value.trim()) { error.value = '需要名称'; return }
  try {
    await invoke('crypto_ca_import', { name: importCaName.value, certPem: importCertPem.value, keyPem: importKeyPem.value })
    importCaName.value = ''; importCertPem.value = ''; importKeyPem.value = ''
    await loadCaList()
  } catch (e) { error.value = '导入失败: ' + (e?.message || e) }
}
async function saveText(content, defName) {
  try { const p = await save({ defaultPath: defName }); if (p) { await invoke('write_file_content', { path: p, content }); error.value = '已保存: ' + p } }
  catch (e) { error.value = '保存失败: ' + (e?.message || e) }
}

// ---- 受管资产库：私钥 / CSR / CRL（持久化 + 查看/导入/导出/删除）----
const assetList = ref([])
async function loadAssets() { try { assetList.value = await invoke('crypto_asset_list', { kind: null }) } catch (_) {} }
async function saveAsset(kind, name, algo, pem, pem2) {
  if (!pem) { error.value = '无内容可存'; return }
  try { await invoke('crypto_asset_save', { kind, name, algo: algo || null, pem, pem2: pem2 || null }); await loadAssets(); error.value = '已存入库' }
  catch (e) { error.value = '存库失败: ' + (e?.message || e) }
}
async function importAsset(kind) {
  const sel = await open({ multiple: false, directory: false, title: '选择要导入的 ' + kind + ' 文件' })
  if (!sel) return
  const path = typeof sel === 'string' ? sel : sel?.path
  try {
    const txt = new TextDecoder().decode(await readFile(path))
    const name = path.replace(/\\/g, '/').split('/').pop() || kind
    await saveAsset(kind, name, '', txt, '')
  } catch (e) { error.value = '导入失败: ' + (e?.message || e) }
}
async function exportAsset(a) {
  try {
    const r = await invoke('crypto_asset_get', { id: a.id })
    const ext = a.kind === 'key' ? 'key' : (a.kind === 'csr' ? 'csr' : 'crl.pem')
    await saveText(r.pem, (a.name || a.kind) + '.' + ext)
    if (r.pem2) await saveText(r.pem2, (a.name || a.kind) + '.pub')
  } catch (e) { error.value = '导出失败: ' + (e?.message || e) }
}
async function viewAsset(a) {
  try { const r = await invoke('crypto_asset_get', { id: a.id }); await openView(`${assetKindLabel[a.kind] || a.kind} · ${a.name}`, r.pem) }
  catch (e) { error.value = '查看失败: ' + (e?.message || e) }
}
async function deleteAsset(a) {
  if (!window.confirm(`删除资产「${a.name}」？`)) return
  try { await invoke('crypto_asset_delete', { id: a.id }); await loadAssets() } catch (e) { error.value = '删除失败: ' + (e?.message || e) }
}
const assetKindLabel = { key: '私钥', csr: 'CSR', crl: 'CRL' }

// ============ PFX：打包 + 解包外部 .pfx/.p12 ============
const pfxKey = ref(''); const pfxCerts = ref(''); const pfxBuildPwd = ref(''); const pfxBuildLoading = ref(false)
const pfxUnpackPwd = ref(''); const pfxUnpacking = ref(false)
const pfxName = ref(''); const pfxKeys = ref([]); const pfxCertsOut = ref([])
async function buildPfx() {
  error.value = ''
  const certs = splitPem(pfxCerts.value)
  if (!pfxKey.value.trim() || !certs.length) { error.value = '需要私钥 + 至少一张证书'; return }
  pfxBuildLoading.value = true
  try {
    const b64 = await invoke('crypto_pfx_build', { keyPem: pfxKey.value, certsPem: certs, password: pfxBuildPwd.value, name: 'guaxi' })
    const path = await save({ defaultPath: 'bundle.pfx', filters: [{ name: 'PFX', extensions: ['pfx', 'p12'] }] })
    if (path) { await invoke('write_file_bytes_base64', { path, base64: b64 }); error.value = '已保存 PFX: ' + path }
  } catch (e) { error.value = '打包失败: ' + (e?.message || e) } finally { pfxBuildLoading.value = false }
}
// 解包：选一个外部 .pfx/.p12，用密码拆出全部私钥与证书，可逐个查看/保存。
async function unpackPfxFile() {
  error.value = ''; pfxKeys.value = []; pfxCertsOut.value = []
  const sel = await open({ multiple: false, directory: false, title: '选择要解包的 PFX/P12 文件', filters: [{ name: 'PKCS#12', extensions: ['pfx', 'p12'] }] })
  if (!sel) return
  const path = typeof sel === 'string' ? sel : sel?.path
  pfxName.value = String(path).replace(/\\/g, '/').split('/').pop() || 'bundle'
  pfxUnpacking.value = true
  try {
    const bytes = await readFile(path)
    const r = await invoke('crypto_pfx_parse', { dataB64: u8ToB64(bytes), password: pfxUnpackPwd.value || '' })
    pfxKeys.value = r.keys_pem || []
    pfxCertsOut.value = r.certs_pem || []
    if (!pfxKeys.value.length && !pfxCertsOut.value.length) error.value = '包里没有可提取的私钥/证书（密码是否正确？）'
  } catch (e) { error.value = '解包失败: ' + (e?.message || e) + '（多为密码不对）' } finally { pfxUnpacking.value = false }
}
function exportPem(pem, name) { return saveText(pem, name) }

const binReport = ref(null); const binLoading = ref(false)

// ============ 对称加解密：覆盖库内支持的全部算法与模式 ============
// 分组密码（可用 ecb/cbc/cfb/ofb/ctr；ecb/cbc 可 PKCS7 填充）
const SYM_BLOCK = [
  { value: 'aes128', label: 'AES-128', key: 16, iv: 16, block: 16 },
  { value: 'aes192', label: 'AES-192', key: 24, iv: 16, block: 16 },
  { value: 'aes256', label: 'AES-256', key: 32, iv: 16, block: 16 },
  { value: 'sm4', label: 'SM4（国密）', key: 16, iv: 16, block: 16 },
  { value: 'camellia128', label: 'Camellia-128', key: 16, iv: 16, block: 16 },
  { value: 'camellia192', label: 'Camellia-192', key: 24, iv: 16, block: 16 },
  { value: 'camellia256', label: 'Camellia-256', key: 32, iv: 16, block: 16 },
  { value: 'twofish', label: 'Twofish', key: 32, iv: 16, block: 16 },
  { value: 'serpent', label: 'Serpent', key: 32, iv: 16, block: 16 },
  { value: 'des', label: 'DES（64 位分组，弱）', key: 8, iv: 8, block: 8 },
  { value: '3des', label: '3DES / TDEA-3key', key: 24, iv: 8, block: 8 },
  { value: 'blowfish', label: 'Blowfish', key: 16, iv: 8, block: 8 },
  { value: 'cast5', label: 'CAST5', key: 16, iv: 8, block: 8 },
  { value: 'rc2', label: 'RC2', key: 16, iv: 8, block: 8 },
  { value: 'idea', label: 'IDEA', key: 16, iv: 8, block: 8 },
]
// 流密码（无模式、无填充）
const SYM_STREAM = [
  { value: 'chacha20', label: 'ChaCha20（流）', key: 32, iv: 12 },
  { value: 'salsa20', label: 'Salsa20（流）', key: 32, iv: 8 },
]
// AEAD（带认证标签，可填 AAD）
const SYM_AEAD = [
  { value: 'aes128gcm', label: 'AES-128-GCM（AEAD）', key: 16, iv: 12 },
  { value: 'aes256gcm', label: 'AES-256-GCM（AEAD）', key: 32, iv: 12 },
  { value: 'chacha20poly1305', label: 'ChaCha20-Poly1305（AEAD）', key: 32, iv: 12 },
]
const symAlgoOptions = [
  { type: 'group', label: '分组密码（ECB/CBC/CFB/OFB/CTR）', children: SYM_BLOCK.map(a => ({ label: a.label, value: a.value })) },
  { type: 'group', label: '流密码', children: SYM_STREAM.map(a => ({ label: a.label, value: a.value })) },
  { type: 'group', label: 'AEAD 认证加密', children: SYM_AEAD.map(a => ({ label: a.label, value: a.value })) },
]
const symModeOptions = [
  { label: 'CBC（需 IV，可填充）', value: 'cbc' },
  { label: 'ECB（无需 IV，可填充，弱）', value: 'ecb' },
  { label: 'CFB（需 IV，块对齐）', value: 'cfb' },
  { label: 'OFB（需 IV，流式）', value: 'ofb' },
  { label: 'CTR（需 IV，流式）', value: 'ctr' },
]
const symAlgo = ref('aes256'); const symMode = ref('cbc'); const symKey = ref(''); const symIv = ref(''); const symAad = ref('')
const symData = ref(''); const symTextInput = ref(true); const symOut = ref('')
const cryptTab = ref('sym'); const symPad = ref(true)
const symEntry = computed(() => [...SYM_BLOCK, ...SYM_STREAM, ...SYM_AEAD].find(a => a.value === symAlgo.value) || SYM_BLOCK[0])
const symIsBlock = computed(() => SYM_BLOCK.some(a => a.value === symAlgo.value))
const symIsAead = computed(() => SYM_AEAD.some(a => a.value === symAlgo.value))
// ECB 不需要 IV；流密码/AEAD 也没有 IV 概念（用 nonce）
const symNeedsIv = computed(() => symIsBlock.value && symMode.value !== 'ecb')
function symKeyLen() { return symEntry.value.key }
function symIvLen() { return symEntry.value.iv || 16 }
function randHex(n) { const u = new Uint8Array(n); crypto.getRandomValues(u); return Array.from(u, b => b.toString(16).padStart(2, '0')).join('') }
function genSymKey() { symKey.value = randHex(symKeyLen()) }
function genSymIv() { symIv.value = randHex(symIvLen()) }
async function runSym(encrypt) {
  error.value = ''; symOut.value = ''
  const dataB64 = encrypt && symTextInput.value ? strToB64(symData.value) : symData.value.trim()
  try {
    const r = await invoke('crypto_sym', {
      algo: symAlgo.value,
      mode: symIsBlock.value ? symMode.value : 'stream',
      keyHex: symKey.value,
      ivHex: symNeedsIv.value || !symIsBlock.value ? (symIv.value || '') : '',
      dataB64, encrypt,
      aadHex: symAad.value || null,
      padding: symIsBlock.value && (symMode.value === 'ecb' || symMode.value === 'cbc') ? symPad.value : null,
    })
    symOut.value = encrypt ? ('密文(base64):\n' + r) : ('明文:\n' + b64ToText(r) + '\n\n(base64): ' + r)
  } catch (e) { error.value = (encrypt ? '加密' : '解密') + '失败: ' + (e?.message || e) }
}

// RSA 加解密
const rsaScheme = ref('oaep-sha256'); const rsaKey = ref(''); const rsaData = ref(''); const rsaTextInput = ref(true); const rsaOut = ref('')
// RSA 加解密方案：库支持 PKCS#1 v1.5 与 OAEP(SHA-1/256/384/512)。
const rsaSchemeOptions = [
  { label: 'OAEP + SHA-256（推荐）', value: 'oaep-sha256' },
  { label: 'OAEP + SHA-384', value: 'oaep-sha384' },
  { label: 'OAEP + SHA-512', value: 'oaep-sha512' },
  { label: 'OAEP + SHA-1（兼容老系统）', value: 'oaep-sha1' },
  { label: 'PKCS#1 v1.5（老式，兼容）', value: 'pkcs1' },
]
async function runRsa(op) {
  error.value = ''; rsaOut.value = ''
  const dataB64 = op === 'encrypt' && rsaTextInput.value ? strToB64(rsaData.value) : rsaData.value.trim()
  try {
    const r = await invoke('crypto_rsa_crypt', { op, keyPem: rsaKey.value, dataB64, scheme: rsaScheme.value })
    rsaOut.value = op === 'encrypt' ? ('密文(base64):\n' + r) : ('明文:\n' + b64ToText(r) + '\n\n(base64): ' + r)
  } catch (e) { error.value = 'RSA ' + (op === 'encrypt' ? '加密' : '解密') + '失败: ' + (e?.message || e) }
}

// 签名/验签
const signScheme = ref('ecdsa-p256'); const signKey = ref(''); const signData = ref(''); const signTextInput = ref(true)
const signSig = ref(''); const signOut = ref(''); const signVerifyResult = ref(null)
const signSchemeOptions = [
  { label: 'RSA PKCS#1 v1.5 + SHA-256', value: 'rsa-pkcs1-sha256' },
  { label: 'RSA PKCS#1 v1.5 + SHA-384', value: 'rsa-pkcs1-sha384' },
  { label: 'RSA PKCS#1 v1.5 + SHA-512', value: 'rsa-pkcs1-sha512' },
  { label: 'RSA-PSS + SHA-256', value: 'rsa-pss-sha256' },
  { label: 'RSA-PSS + SHA-384', value: 'rsa-pss-sha384' },
  { label: 'RSA-PSS + SHA-512', value: 'rsa-pss-sha512' },
  { label: 'ECDSA P-256 + SHA-256', value: 'ecdsa-p256' },
  { label: 'ECDSA P-384 + SHA-384', value: 'ecdsa-p384' },
  { label: 'Ed25519', value: 'ed25519' },
]
async function runSign() {
  error.value = ''; signOut.value = ''
  const dataB64 = signTextInput.value ? strToB64(signData.value) : signData.value.trim()
  try {
    const sig = await invoke('crypto_sign', { keyPem: signKey.value.trim() || genPrivPem.value, dataB64, scheme: signScheme.value })
    signOut.value = '签名(base64):\n' + sig; signSig.value = sig
  } catch (e) { error.value = '签名失败: ' + (e?.message || e) }
}
async function runVerify() {
  error.value = ''; signVerifyResult.value = null
  const dataB64 = signTextInput.value ? strToB64(signData.value) : signData.value.trim()
  try { signVerifyResult.value = await invoke('crypto_verify', { pubPem: signKey.value.trim() || genPubPem.value, dataB64, sigB64: signSig.value, scheme: signScheme.value }) }
  catch (e) { error.value = '验签失败: ' + (e?.message || e) }
}

// 散列
const hashAlgo = ref('sha256'); const hashData = ref(''); const hashTextInput = ref(true); const hashOut = ref('')
const hashAlgoOptions = [{ label: 'MD5', value: 'md5' }, { label: 'SHA-1', value: 'sha1' }, { label: 'SHA-256', value: 'sha256' }, { label: 'SHA-384', value: 'sha384' }, { label: 'SHA-512', value: 'sha512' }]
async function runHash() {
  error.value = ''; hashOut.value = ''
  const dataB64 = hashTextInput.value ? strToB64(hashData.value) : hashData.value.trim()
  try { hashOut.value = hashAlgo.value + ': ' + await invoke('crypto_hash', { algo: hashAlgo.value, dataB64 }) }
  catch (e) { error.value = '散列失败: ' + (e?.message || e) }
}

async function pickBinary() {
  error.value = ''
  const sel = await open({ multiple: false, directory: false }); if (!sel) return
  const path = typeof sel === 'string' ? sel : sel?.path
  binLoading.value = true
  try { binReport.value = await invoke('binary_analyze', { path }) }
  catch (e) { error.value = '分析失败: ' + (e?.message || e) } finally { binLoading.value = false }
}

const coreProgram = ref(''); const coreFile = ref(''); const coreOut = ref(''); const coreLoading = ref(false)
const coreFontSize = ref(13)
function setCoreFontSize(v) { coreFontSize.value = Math.min(28, Math.max(9, v)) }
async function analyzeCore() {
  if (!profileId.value || !coreProgram.value || !coreFile.value) { error.value = '需要档案 + 程序 + core'; return }
  coreLoading.value = true; error.value = ''
  try { const r = await invoke('gdb_coredump_analyze', { profileId: profileId.value, program: coreProgram.value, core: coreFile.value }); coreOut.value = r.output }
  catch (e) { error.value = '分析失败: ' + (e?.message || e) } finally { coreLoading.value = false }
}

const execCmd = ref(''); const execOut = ref(''); const execLoading = ref(false); const execSaved = ref('')
async function runExec() {
  if (!profileId.value || !execCmd.value) { error.value = '需要档案 + 命令'; return }
  execLoading.value = true; error.value = ''
  try { const r = await invoke('ssh_exec_capture', { profileId: profileId.value, command: execCmd.value }); execOut.value = r.output; execSaved.value = r.saved_path }
  catch (e) { error.value = '执行失败: ' + (e?.message || e) } finally { execLoading.value = false }
}
function importCapture() { if (execSaved.value) emit('import-file', execSaved.value) }

const periodSecs = ref(10); const collectorId = ref(''); const collectorPath = ref('')
async function startPeriodic() {
  if (!profileId.value || !execCmd.value) { error.value = '需要档案 + 命令'; return }
  try {
    const r = await invoke('ssh_start_periodic', { profileId: profileId.value, command: execCmd.value, intervalSecs: periodSecs.value, label: 'periodic' })
    collectorId.value = r.id; collectorPath.value = r.path
  } catch (e) { error.value = '启动采集失败: ' + (e?.message || e) }
}
async function stopPeriodic() {
  if (collectorId.value) {
    await invoke('ssh_stop_periodic', { id: collectorId.value }).catch(() => {})
    const p = collectorPath.value; collectorId.value = ''
    if (p) emit('import-file', p)
  }
}

async function sendToEditor(content) {
  if (!content) return
  try { await invoke('emit_template_insert', { content }) }
  catch (e) { error.value = '发送失败: ' + (e?.message || e) }
}

onMounted(async () => { try { profiles.value = await invoke('ssh_list_profiles') } catch (e) { logger.error('profiles', e) } ; loadCaList(); loadAssets() })

// J：多机时间轴对齐——导入多份日志，把各自快照按时间戳排到同一条时间线。
const multiLogs = ref([]) // { name, sessionId, points: [[ts, laneIndex], ...] }
async function addMultiLog() {
  error.value = ''
  const sel = await open({ multiple: false, directory: false, title: '选择日志文件' })
  if (!sel) return
  const path = typeof sel === 'string' ? sel : sel?.path
  try {
    const res = await invoke('log_import_file', { path })
    const lane = multiLogs.value.length
    const name = path.replace(/\\/g, '/').split('/').pop() || path
    const points = (res.frames || [])
      .filter(f => f.timestamp_ms != null)
      .map(f => [f.timestamp_ms, lane])
    multiLogs.value.push({ name, sessionId: res.session_id, points })
  } catch (e) { error.value = '导入失败: ' + (e?.message || e) }
}
function clearMultiLogs() { multiLogs.value = [] }
const multiChartOption = computed(() => {
  if (!multiLogs.value.length) return null
  return {
    animation: false,
    grid: { left: 140, right: 20, top: 20, bottom: 50 },
    tooltip: { trigger: 'item', formatter: p => new Date(p.value[0]).toLocaleString() },
    legend: { show: false },
    xAxis: { type: 'time', name: '时间' },
    yAxis: {
      type: 'category',
      data: multiLogs.value.map(m => m.name),
    },
    dataZoom: [{ type: 'inside' }, { type: 'slider', height: 18, bottom: 12 }],
    series: multiLogs.value.map((m, i) => ({
      name: m.name, type: 'scatter', symbolSize: 6,
      data: m.points.map(pt => [pt[0], i]),
    })),
  }
})
</script>

<style scoped>
.tb-root { height: 100%; display: flex; flex-direction: column; min-height: 0; }
.tb-tabs { height: 100%; display: flex; flex-direction: column; }
.tb-tabs :deep(.tb-pane) { height: 100%; overflow: auto; padding: 12px; }
.tb-sec { margin-bottom: 18px; }
.tb-h { font-size: 12px; font-weight: 600; margin-bottom: 8px; }
.tb-row { display: flex; gap: 8px; align-items: center; flex-wrap: wrap; margin-bottom: 8px; }
.tb-res { font-size: 12px; font-family: var(--guaxi-mono-font, monospace); color: var(--guaxi-text-secondary, #6b7280); }
.tb-err { font-size: 12px; color: #e5484d; }
.tb-kv { border-collapse: collapse; font-family: var(--guaxi-mono-font, monospace); font-size: 12px; margin-top: 8px; }
.tb-kv td { padding: 3px 12px 3px 0; }
.tb-kv td:first-child { color: var(--guaxi-text-secondary, #9ca3af); }
.tb-table { width: 100%; border-collapse: collapse; font-size: 12px; }
.tb-table th, .tb-table td { text-align: left; padding: 4px 8px; border-bottom: 1px solid var(--guaxi-border-weak, rgba(0,0,0,0.06)); }
.tb-ent { position: relative; background: var(--guaxi-border-weak, rgba(0,0,0,0.08)); border-radius: 3px; height: 16px; min-width: 120px; }
.tb-ent-bar { height: 100%; border-radius: 3px; }
.tb-ent span { position: absolute; left: 6px; top: 0; font-size: 11px; line-height: 16px; }
.tb-out { background: #1e1e1e; color: #d4d4d4; font-family: var(--guaxi-mono-font, monospace); font-size: 12px; padding: 8px; border-radius: 6px; white-space: pre-wrap; word-break: break-all; min-height: 120px; max-height: 50vh; overflow: auto; }
.tb-hint { font-size: 11px; color: var(--guaxi-text-secondary, #9ca3af); margin-top: 6px; }
.tb-multi-chart { width: 100%; height: 360px; }

/* ---- 美化层 ---- */
.tw-tab { display: inline-flex; align-items: center; gap: 7px; font-weight: 500; }
.tb-root { font-family: var(--tw-ui-font); }
.tb-tabs :deep(.n-tabs-tab) { font-family: var(--tw-ui-font); font-weight: 500; }
.tb-tabs :deep(.n-tabs-tab--active) { color: var(--tw-accent) !important; }
.tb-tabs :deep(.n-tabs-bar) { background: var(--tw-accent) !important; height: 2.5px; border-radius: 2px; }
.tb-h { color: var(--tw-accent); letter-spacing: .02em; display: inline-flex; align-items: center; gap: 6px; }
.tb-sec { padding: 14px 16px; margin-bottom: 8px; border: 1px solid color-mix(in srgb, var(--guaxi-border) 60%, transparent); border-radius: var(--tw-radius); background: color-mix(in srgb, var(--guaxi-panel-bg) 40%, transparent); }
.tb-kv { font-family: var(--tw-mono-font); }
.tb-kv td:first-child { color: var(--tw-accent); font-weight: 600; }
.tb-res { font-family: var(--tw-mono-font); }
.tb-out { background: var(--tw-code-bg); color: var(--tw-code-fg); font-family: var(--tw-mono-font); border-radius: var(--tw-radius-sm); line-height: 1.55; }
.tb-table th { text-transform: uppercase; letter-spacing: .04em; font-size: 11px; color: var(--guaxi-text-secondary); }

/* Coredump：主分析区占满整个 pane 高度，输出字号可调。 */
.tb-sec-fill { height: 100%; display: flex; flex-direction: column; min-height: 0; margin-bottom: 0; }
.tb-sec-fill .tb-row { flex: 0 0 auto; }
.tb-out-fill { flex: 1 1 auto; min-height: 0; max-height: none; height: auto; margin: 0; }
.tb-fontctl { display: inline-flex; align-items: center; gap: 4px; margin-left: auto; padding: 2px 4px; border: 1px solid var(--guaxi-border); border-radius: 999px; }
.tb-fbtn { display: inline-flex; align-items: center; justify-content: center; width: 22px; height: 22px; border: none; background: transparent; color: var(--guaxi-text-secondary); cursor: pointer; border-radius: 50%; }
.tb-fbtn:hover { background: var(--tw-accent-soft); color: var(--tw-accent); }
.tb-fsz { min-width: 20px; text-align: center; font-size: 12px; font-family: var(--tw-mono-font); color: var(--tw-accent); }

/* ===== 证书子标签：顶部标签栏固定，只有内容区滚动 ===== */
/* 证书页自己不滚（交给子标签的内容区滚），避免出现两层滚动条把标签栏顶走。 */
.tb-tabs :deep(.cx-pane) { overflow: hidden; padding: 0; }
/* 命令下发页同样是自管布局（自带左右分栏与各自滚动区）。 */
.tb-tabs :deep(.cd-pane) { overflow: hidden; padding: 0; }
.cx-tabs { height: 100%; display: flex; flex-direction: column; min-height: 0; }
.cx-tabs :deep(.n-tabs-nav) { flex: 0 0 auto; position: sticky; top: 0; z-index: 3; padding: 8px 12px 0; background: var(--guaxi-content-bg); }
.cx-tabs :deep(.n-tab-pane) { flex: 1 1 auto; min-height: 0; overflow: auto; padding: 8px 12px 12px; box-sizing: border-box; }

/* ===== 图标按钮（带 tooltip） ===== */
.tb-ib { display: inline-flex; align-items: center; justify-content: center; width: 26px; height: 26px; flex: 0 0 auto; border: none; border-radius: 6px; background: transparent; color: var(--guaxi-text-secondary); cursor: pointer; transition: background .12s, color .12s; vertical-align: middle; }
.tb-ib:hover:not(:disabled) { background: var(--tw-accent-soft); color: var(--tw-accent); }
.tb-ib.primary { color: var(--tw-accent); border: 1px solid color-mix(in srgb, var(--tw-accent) 45%, transparent); }
.tb-ib.danger:hover:not(:disabled) { background: rgba(229,72,77,0.14); color: #e5484d; }
.tb-ib:disabled { opacity: .35; cursor: default; }
.tb-ib.lg { width: 32px; height: 32px; }
.tb-ib.tb-ok { color: #30a46c; }
.tb-spin { animation: tb-spin 1s linear infinite; }
@keyframes tb-spin { to { transform: rotate(360deg); } }
.tb-ops { display: flex; gap: 2px; align-items: center; white-space: nowrap; }
.tb-help { color: var(--guaxi-text-secondary); cursor: help; margin-left: 4px; vertical-align: -2px; }
.tb-help:hover { color: var(--tw-accent); }
.tb-grp { display: flex; align-items: center; gap: 5px; margin: 8px 0 4px; font-size: 12px; font-weight: 600; color: var(--tw-accent); }
.tb-grp-ico { color: #e0a53f; }
.tb-grp-n { font-size: 10px; padding: 0 6px; border-radius: 999px; background: var(--tw-accent-soft); }

/* ===== 查看弹窗 ===== */
.tb-sub { margin: 10px 0 4px; font-size: 12px; font-weight: 600; color: var(--tw-accent); }
.tv-modal { display: flex; flex-direction: column; max-height: 82vh; background: var(--guaxi-panel-bg); border: 1px solid var(--guaxi-border); border-radius: var(--tw-radius); box-shadow: var(--tw-elev); overflow: hidden; font-family: var(--tw-ui-font); }
.tv-head { display: flex; align-items: center; gap: 8px; padding: 12px 14px; border-bottom: 1px solid var(--guaxi-border); }
.tv-head-ico { color: var(--tw-accent); flex: 0 0 auto; }
.tv-title { flex: 1 1 auto; font-weight: 600; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.tv-tabs { display: flex; gap: 2px; padding: 8px 12px 0; border-bottom: 1px solid var(--guaxi-border); flex: 0 0 auto; }
.tv-tab { padding: 7px 16px; border: none; background: transparent; color: var(--guaxi-text-secondary); cursor: pointer; font-size: 12.5px; font-family: var(--tw-ui-font); border-bottom: 2px solid transparent; border-radius: 6px 6px 0 0; }
.tv-tab:hover { color: var(--tw-accent); }
.tv-tab.active { color: var(--tw-accent); border-bottom-color: var(--tw-accent); font-weight: 600; }
.tv-body { flex: 1 1 auto; min-height: 0; overflow: auto; padding: 12px 14px; }
.tv-kv { margin: 0; }
.tv-kv td { padding: 4px 14px 4px 0; vertical-align: top; }
.tv-kv td:first-child { white-space: nowrap; }
.tv-v { font-family: var(--tw-mono-font); word-break: break-all; }
.tv-sub { margin: 12px 0 6px; font-size: 12px; font-weight: 600; color: var(--tw-accent); }
.tv-sans { display: flex; flex-wrap: wrap; gap: 6px; }
.tv-san { font-family: var(--tw-mono-font); font-size: 12px; padding: 2px 8px; border-radius: 999px; background: var(--tw-accent-soft); color: var(--tw-accent); }
.tv-mono { font-family: var(--tw-mono-font); font-size: 12px; }
.tv-fp { display: flex; align-items: center; gap: 6px; word-break: break-all; }
.tv-bar { display: flex; align-items: center; gap: 8px; margin-bottom: 8px; }
.tv-hint { flex: 1 1 auto; font-size: 12px; color: var(--guaxi-text-secondary); }
.tv-ok { font-size: 12px; color: #30a46c; }
.tv-err { color: #e5484d; font-size: 12px; }
.tv-pre { margin: 0; padding: 10px; border-radius: var(--tw-radius-sm); background: var(--tw-code-bg); color: var(--tw-code-fg); font-family: var(--tw-mono-font); font-size: 12px; line-height: 1.5; white-space: pre-wrap; word-break: break-all; }
</style>
