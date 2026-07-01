/**
 * e2e.js — Script de Testes Fim-a-Fim (E2E) para o Contrato Bairrou Coupons
 *
 * Requer: @stellar/stellar-sdk >= 16.x, dotenv
 * Uso:    node e2e.js
 *
 * Fluxo testado:
 *   1. Admin inicializa a campanha com o merchant como redeemer autorizado
 *   2. Admin adiciona o usuário na whitelist de elegibilidade
 *   3. Usuário faz o claim de um cupom
 *   4. Testa que um segundo claim é corretamente bloqueado
 *   5. Consulta o estado do cupom no ledger
 *   6. Resgate multi-auth: user + merchant assinam a transação de redeem
 *   7. Verifica que o cupom foi removido da storage (burned) após o resgate
 */
import {
  rpc,
  TransactionBuilder,
  Keypair,
  xdr,
  Address,
  Contract,
  Networks,
  authorizeEntry,
} from '@stellar/stellar-sdk';
import dotenv from 'dotenv';
import path from 'path';
import { fileURLToPath } from 'url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
dotenv.config({ path: path.resolve(__dirname, '../.env') });

const server = new rpc.Server(process.env.RPC_URL);
const networkPassphrase = process.env.NETWORK_PASSPHRASE;
const contractId = process.env.CONTRACT_ID;

const adminKeypair   = Keypair.fromSecret(process.env.ADMIN_SECRET);
const userKeypair    = Keypair.fromSecret(process.env.USER_SECRET);
const merchantKeypair = Keypair.fromSecret(process.env.MERCHANT_SECRET);

const adminAddress    = adminKeypair.publicKey();
const userAddress     = userKeypair.publicKey();
const merchantAddress = merchantKeypair.publicKey();

if (!contractId) {
  console.error("❌ CONTRACT_ID não configurado. Execute 'node deploy.js' primeiro.");
  process.exit(1);
}

const contract = new Contract(contractId);

// ─── Utilitários de XDR ─────────────────────────────────────────────────────

/**
 * Extrai o valor JS de um ScVal de forma segura.
 * Usa o nome do switch (string) ao invés de comparar objetos XDR diretamente,
 * evitando o erro "Bad union switch" em versões incompatíveis.
 */
function scValToJs(val) {
  const t = val.switch().name;
  if (t === 'scvU32')    return val.u32();
  if (t === 'scvI32')    return val.i32();
  if (t === 'scvU64')    return val.u64().toString();
  if (t === 'scvI64')    return val.i64().toString();
  if (t === 'scvString') return val.str().toString();
  if (t === 'scvSymbol') return val.sym().toString();
  if (t === 'scvBool')   return val.b();
  if (t === 'scvVoid')   return null;
  if (t === 'scvAddress') return Address.fromScVal(val).toString();
  if (t === 'scvVec')    return val.vec().map(scValToJs);
  if (t === 'scvMap') {
    const obj = {};
    for (const entry of val.map()) {
      obj[entry.key().sym().toString()] = scValToJs(entry.val());
    }
    return obj;
  }
  // Fallback genérico para enums de contrato (scvEnum / scvU32 wrappado em map)
  return val.value !== undefined ? val.value() : val.toString();
}

// ─── Envio de Transações ─────────────────────────────────────────────────────

async function pollTx(hash) {
  const maxAttempts = 45;
  for (let attempt = 0; attempt < maxAttempts; attempt++) {
    const response = await server.getTransaction(hash);
    const status = response.status;
    if (status === 'SUCCESS') return response;
    if (status === 'FAILED') {
      throw new Error(
        `Transação falhou. Hash: ${hash}\nResultXDR: ${response.resultXdr ?? 'N/A'}`
      );
    }
    // 'NOT_FOUND' e 'PENDING' são estados temporários durante a ingestão
    console.log(`  Aguardando... status: ${status} (${attempt + 1}/${maxAttempts})`);
    await new Promise(r => setTimeout(r, 1500));
  }
  throw new Error(`Timeout aguardando transação ${hash}`);
}

/**
 * Simula, monta, assina e envia uma transação simples (1 assinante).
 */
async function submitTx(builder, signer) {
  let tx = builder.build();

  const simResponse = await server.simulateTransaction(tx);
  if (rpc.Api.isSimulationError(simResponse)) {
    const msg = simResponse.error?.message ?? JSON.stringify(simResponse.error ?? simResponse);
    throw new Error(`Falha na simulação: ${msg}`);
  }

  tx = rpc.assembleTransaction(tx, simResponse).build();
  tx.sign(signer);

  const response = await server.sendTransaction(tx);
  if (response.status === 'ERROR') {
    throw new Error(
      `Erro ao enviar transação: ${response.errorResultXdr ?? response.status}`
    );
  }

  return pollTx(response.hash);
}

// ─── Etapas do Fluxo E2E ──────────────────────────────────────────────────────

async function initializeCampaign() {
  const adminSource = await server.getAccount(adminAddress);
  const builder = new TransactionBuilder(adminSource, { fee: '200', networkPassphrase })
    .addOperation(contract.call(
      'initialize',
      Address.fromString(adminAddress).toScVal(),
      xdr.ScVal.scvU32(200),
      xdr.ScVal.scvU32(5),
      xdr.ScVal.scvU64(xdr.Uint64.fromString('1900000000')),
      xdr.ScVal.scvString('ipfs://QmCampaignMetadataExample'),
      xdr.ScVal.scvVec([Address.fromString(merchantAddress).toScVal()])
    ))
    .setTimeout(30);

  await submitTx(builder, adminKeypair);
  console.log('  ✅ Campanha inicializada no ledger!');
}

async function printCampaignInfo() {
  const userSource = await server.getAccount(userAddress);
  const builder = new TransactionBuilder(userSource, { fee: '100', networkPassphrase })
    .addOperation(contract.call('get_campaign_info'))
    .setTimeout(30);

  const sim = await server.simulateTransaction(builder.build());
  if (rpc.Api.isSimulationError(sim)) throw new Error('Falha ao ler dados da campanha');

  const info = scValToJs(sim.result.retval);
  console.log('  Dados da Campanha:', JSON.stringify(info, null, 2));
}

async function whitelistUser() {
  const adminSource = await server.getAccount(adminAddress);
  const builder = new TransactionBuilder(adminSource, { fee: '200', networkPassphrase })
    .addOperation(contract.call(
      'add_eligible_user',
      Address.fromString(userAddress).toScVal()
    ))
    .setTimeout(30);

  await submitTx(builder, adminKeypair);
  console.log('  ✅ Usuário adicionado à whitelist!');
}

async function claimCoupon() {
  const userSource = await server.getAccount(userAddress);
  const builder = new TransactionBuilder(userSource, { fee: '200', networkPassphrase })
    .addOperation(contract.call(
      'claim',
      Address.fromString(userAddress).toScVal()
    ))
    .setTimeout(30);

  const result = await submitTx(builder, userKeypair);
  const couponId = result.returnValue.u32();
  console.log(`  ✅ Cupom #${couponId} emitido para o usuário!`);
  return couponId;
}

async function printCouponInfo(couponId) {
  const userSource = await server.getAccount(userAddress);
  const builder = new TransactionBuilder(userSource, { fee: '100', networkPassphrase })
    .addOperation(contract.call('get_coupon', xdr.ScVal.scvU32(couponId)))
    .setTimeout(30);

  const sim = await server.simulateTransaction(builder.build());
  if (rpc.Api.isSimulationError(sim)) throw new Error('Erro ao consultar cupom');

  const coupon = scValToJs(sim.result.retval);
  console.log(`  Estado do cupom #${couponId}:`, JSON.stringify(coupon, null, 2));
}

async function testDoubleClaim() {
  const userSource = await server.getAccount(userAddress);
  const builder = new TransactionBuilder(userSource, { fee: '100', networkPassphrase })
    .addOperation(contract.call('claim', Address.fromString(userAddress).toScVal()))
    .setTimeout(30);

  const sim = await server.simulateTransaction(builder.build());
  if (rpc.Api.isSimulationError(sim)) {
    console.log('  ✅ Claim duplicado corretamente bloqueado pela rede!');
  } else {
    throw new Error('❌ O contrato permitiu a simulação de um claim duplo!');
  }
}

/**
 * Executa o redeem com multi-auth: user (fee payer) + merchant (auth entry).
 *
 * No SDK v16 o fluxo correto é:
 *   1. Construir tx bruta
 *   2. Simular → obter auth entries
 *   3. Assinar as auth entries do merchant com authorizeEntry()
 *   4. Montar a tx final com assembleTransaction()
 *   5. Assinar o envelope com a chave do pagador (user)
 *   6. Enviar
 */
async function redeemCoupon(couponId) {
  const userSource = await server.getAccount(userAddress);

  // 1. Construir transação bruta
  const builder = new TransactionBuilder(userSource, { fee: '500', networkPassphrase })
    .addOperation(contract.call(
      'redeem',
      Address.fromString(userAddress).toScVal(),
      xdr.ScVal.scvU32(couponId),
      Address.fromString(merchantAddress).toScVal()
    ))
    .setTimeout(30);

  let tx = builder.build();

  // 2. Simular para gerar auth entries
  console.log('  Simulando transação de resgate...');
  const simResponse = await server.simulateTransaction(tx);
  if (rpc.Api.isSimulationError(simResponse)) {
    const msg = simResponse.error?.message ?? JSON.stringify(simResponse.error ?? simResponse);
    throw new Error(`Falha na simulação do redeem: ${msg}`);
  }

  // 3. Obter o número do ledger atual para validUntil das auth entries
  const latestLedger = await server.getLatestLedger();
  const validUntilLedgerSeq = latestLedger.sequence + 200; // ~200 ledgers de validade

  // 4. Assinar as auth entries que pertencem ao merchant
  // simResponse.result.auth contém as SorobanAuthorizationEntry da simulação
  const authEntries = simResponse.result?.auth ?? [];
  console.log(`  Assinando ${authEntries.length} auth entrie(s) do merchant...`);

  const signedAuthEntries = await Promise.all(
    authEntries.map(async (entry) => {
      // Verificar se a auth entry pertence ao merchant
      const credType = entry.credentials().switch().name;
      if (credType === 'sorobanCredentialsAddress') {
        const entryAddress = Address.fromScAddress(
          entry.credentials().address().address()
        ).toString();
        if (entryAddress === merchantAddress) {
          return authorizeEntry(entry, merchantKeypair, validUntilLedgerSeq, networkPassphrase);
        }
      }
      return entry; // devolver sem modificar entradas que não são do merchant
    })
  );

  // 5. Montar a transação final com as auth entries assinadas
  // Injetamos as auth entries de volta na simulação antes de montar
  simResponse.result.auth = signedAuthEntries;
  tx = rpc.assembleTransaction(tx, simResponse).build();

  // 6. Assinar o envelope com a chave do user (pagador de taxa)
  console.log('  Assinando envelope da transação com a chave do usuário...');
  tx.sign(userKeypair);

  // 7. Enviar
  console.log('  Enviando transação de resgate para a rede...');
  const response = await server.sendTransaction(tx);
  if (response.status === 'ERROR') {
    throw new Error(
      `Submissão do resgate falhou: ${response.errorResultXdr ?? response.status}`
    );
  }

  const result = await pollTx(response.hash);
  const returnValue = result.returnValue;
  // RedemptionStatus é um enum: Redeemed=1, Expired=2 — vem como scvMap/scvU32
  const statusRaw = scValToJs(returnValue);
  const statusName = typeof statusRaw === 'object'
    ? JSON.stringify(statusRaw)
    : (statusRaw === 1 ? 'Redeemed ✅' : 'Expired ⏰');
  console.log(`  ✅ Resgate executado! Status: ${statusName}`);
}

async function verifyBurned(couponId) {
  const userSource = await server.getAccount(userAddress);
  const builder = new TransactionBuilder(userSource, { fee: '100', networkPassphrase })
    .addOperation(contract.call('get_coupon', xdr.ScVal.scvU32(couponId)))
    .setTimeout(30);

  const sim = await server.simulateTransaction(builder.build());
  if (rpc.Api.isSimulationError(sim)) {
    throw new Error('Erro ao simular consulta final de queima');
  }

  const val = sim.result.retval;
  if (val.switch().name === 'scvVoid') {
    console.log('  ✅ Cupom removido permanentemente da storage do ledger (burned)!');
  } else {
    throw new Error(`❌ Cupom #${couponId} ainda está na storage: ${JSON.stringify(scValToJs(val))}`);
  }
}

// ─── Fluxo Principal ─────────────────────────────────────────────────────────

async function main() {
  console.log('====================================================');
  console.log('=== FLUXO DE TESTES E2E NA TESTNET STELLAR       ===');
  console.log('====================================================');
  console.log(`Contrato:  ${contractId}`);
  console.log(`Admin:     ${adminAddress}`);
  console.log(`Usuário:   ${userAddress}`);
  console.log(`Merchant:  ${merchantAddress}`);
  console.log('====================================================');

  try {
    console.log('\n[1/7] Inicializando campanha...');
    await initializeCampaign();

    console.log('\n[2/7] Consultando dados da campanha...');
    await printCampaignInfo();

    console.log('\n[3/7] Adicionando usuário à whitelist de elegibilidade...');
    await whitelistUser();

    console.log('\n[4/7] Usuário reivindicando cupom (claim)...');
    const couponId = await claimCoupon();

    console.log('\n[5/7] Testando bloqueio de claim duplicado...');
    await testDoubleClaim();

    console.log('\n[5.5] Consultando estado do cupom no ledger...');
    await printCouponInfo(couponId);

    console.log('\n[6/7] Resgatando cupom com assinatura dupla (user + merchant)...');
    await redeemCoupon(couponId);

    console.log('\n[7/7] Verificando que o cupom foi removido da storage (burned)...');
    await verifyBurned(couponId);

    console.log('\n====================================================');
    console.log('=== ✅ TODOS OS TESTES E2E CONCLUÍDOS COM SUCESSO ===');
    console.log('====================================================');
  } catch (error) {
    console.error('\n❌ Erro no fluxo E2E:');
    console.error(error.stack ?? error);
    process.exit(1);
  }
}

main();
