import {
  rpc,
  TransactionBuilder,
  Keypair,
  xdr,
  Address,
  Contract,
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
const merchantKeypair = Keypair.fromSecret(process.env.MERCHANT_SECRET);

const adminAddress    = adminKeypair.publicKey();
const merchantAddress = merchantKeypair.publicKey();

if (!contractId) {
  console.error("❌ CONTRACT_ID não configurado.");
  process.exit(1);
}

const contract = new Contract(contractId);

async function pollTx(hash) {
  const maxAttempts = 30;
  for (let attempt = 0; attempt < maxAttempts; attempt++) {
    const response = await server.getTransaction(hash);
    const status = response.status;
    if (status === 'SUCCESS') return response;
    if (status === 'FAILED') {
      throw new Error(`Transação falhou: ${response.resultXdr ?? 'N/A'}`);
    }
    await new Promise(r => setTimeout(r, 1000));
  }
  throw new Error(`Timeout aguardando transação ${hash}`);
}

async function submitTx(builder, signer) {
  let tx = builder.build();
  const simResponse = await server.simulateTransaction(tx);
  if (rpc.Api.isSimulationError(simResponse)) {
    throw new Error(`Falha na simulação: ${JSON.stringify(simResponse.error)}`);
  }
  tx = rpc.assembleTransaction(tx, simResponse).build();
  tx.sign(signer);
  const response = await server.sendTransaction(tx);
  if (response.status === 'ERROR') {
    throw new Error(`Erro ao enviar: ${response.status}`);
  }
  return pollTx(response.hash);
}

async function main() {
  console.log(`Inicializando campanha para o contrato: ${contractId}...`);
  try {
    const adminSource = await server.getAccount(adminAddress);
    const builder = new TransactionBuilder(adminSource, { fee: '200', networkPassphrase })
      .addOperation(contract.call(
        'initialize',
        Address.fromString(adminAddress).toScVal(),
        xdr.ScVal.scvU32(200), // campaign_id
        xdr.ScVal.scvU32(100), // max_supply
        xdr.ScVal.scvU64(xdr.Uint64.fromString('1900000000')), // expiration
        xdr.ScVal.scvString('ipfs://QmCampaignMetadataExample'),
        xdr.ScVal.scvVec([Address.fromString(merchantAddress).toScVal()])
      ))
      .setTimeout(30);

    await submitTx(builder, adminKeypair);
    console.log('✅ Campanha inicializada com sucesso on-chain!');
  } catch (error) {
    console.error('❌ Falha na inicialização:', error.message || error);
  }
}

main();
