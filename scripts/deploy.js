import { execSync } from 'child_process';
import dotenv from 'dotenv';
import fs from 'fs';
import path from 'path';
import { fileURLToPath } from 'url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
dotenv.config({ path: path.resolve(__dirname, '../.env') });

const adminSecret = process.env.ADMIN_SECRET;
const rpcUrl = process.env.RPC_URL;
const networkPassphrase = process.env.NETWORK_PASSPHRASE;

if (!adminSecret || !rpcUrl || !networkPassphrase) {
  console.error("Erro: Variáveis de ambiente ADMIN_SECRET, RPC_URL ou NETWORK_PASSPHRASE não estão configuradas no arquivo .env.");
  process.exit(1);
}

try {
  console.log("=== [1/4] Compilando contrato inteligente em WebAssembly (target wasm32) ===");
  execSync("cargo build --target wasm32-unknown-unknown --release", { stdio: 'inherit' });

  console.log("\n=== [2/4] Otimizando o bytecode Wasm para deploy (limites de tamanho do Soroban) ===");
  execSync("stellar contract optimize --wasm target/wasm32-unknown-unknown/release/bairrou_coupons.wasm", { stdio: 'inherit' });

  console.log("\n=== [3/4] Fazendo o deploy do contrato na Testnet ===");
  const deployCmd = `stellar contract deploy \
    --wasm target/wasm32-unknown-unknown/release/bairrou_coupons.optimized.wasm \
    --rpc-url "${rpcUrl}" \
    --network-passphrase "${networkPassphrase}" \
    --source "${adminSecret}"`;

  console.log("Enviando transação de implantação...");
  const contractId = execSync(deployCmd).toString().trim();
  console.log(`\nContrato implantado com sucesso! ID: ${contractId}`);

  console.log("\n=== [4/4] Atualizando o arquivo .env com o CONTRACT_ID ===");
  const envPath = path.resolve(__dirname, '../.env');
  let envContent = fs.readFileSync(envPath, 'utf8');
  if (envContent.includes("CONTRACT_ID=")) {
    envContent = envContent.replace(/CONTRACT_ID=.*/, `CONTRACT_ID=${contractId}`);
  } else {
    envContent += `\nCONTRACT_ID=${contractId}`;
  }
  fs.writeFileSync(envPath, envContent);
  console.log("Arquivo .env atualizado com sucesso com o novo CONTRACT_ID!");

} catch (error) {
  console.error("\n❌ Falha no processo de deploy:", error.message);
  process.exit(1);
}
