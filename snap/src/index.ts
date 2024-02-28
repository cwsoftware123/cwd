import { sha256 } from "@cosmjs/crypto";
import { Addr, Message, SigningKey, encodeBase64, encodeHex } from "@cwsoftware/cw.js";
import { getBIP44AddressKeyDeriver } from "@metamask/key-tree";
import {
  Component,
  Json,
  OnRpcRequestHandler,
  address,
  divider,
  heading,
  panel,
  row,
  text,
} from "@metamask/snaps-sdk";

/**
 * The BIP-44 coin type of Ethereum, which we use also for CWD chains.
 */
const ETHEREUM_COIN_TYPE = 60;

/**
 * Data that the webapp must provide the Snap in order to sign a transaction.
 */
export type SignTransactionParams = {
  readonly msgs: Message[];
  readonly sender: Addr;
  readonly chainId: string;
  readonly sequence: number;
};

/**
 * Convert a list of messages to Snap components so that they can be displayed
 * on the MetaMask wallet interface.
 * @param msgs Messages to be displayed
 * @returns A list of components based on the messages.
 */
export function displayMessages(msgs: Message[]): Component[] {
  let components: Component[] = [];
  for (const [idx, msg] of msgs.entries()) {
    components.push(divider());
    if ("updateConfig" in msg) {
      const { newCfg } = msg.updateConfig;
      components.concat([
        row(`message #${idx + 1}`, text("update config")),
        row("new config", text(JSON.stringify(newCfg))),
      ]);
    } else if ("transfer" in msg) {
      const { to, coins } = msg.transfer;
      components = components.concat([
        row(`message #${idx + 1}`, text("transfer")),
        row("to", address(to.toString())),
        row("coins", text(JSON.stringify(coins))), // TODO: better stringification of coins
      ]);
    } else if ("storeCode" in msg) {
      const { wasmByteCode } = msg.storeCode;
      const codeHash = sha256(wasmByteCode.bytes);
      const codeSize = Math.floor(wasmByteCode.bytes.length / 1024);
      components = components.concat([
        row(`message #${idx + 1}`, text("store code")),
        row("code hash", text(encodeHex(codeHash))),
        row("code size", text(`${codeSize} kB`)),
      ]);
    } else if ("instantiate" in msg) {
      const { codeHash, msg: initMsg, salt, funds, admin } = msg.instantiate;
      components = components.concat([
        row(`message #${idx + 1}`, text("instantiate")),
        row("code hash", text(codeHash.toHex())),
        row("msg", text(JSON.stringify(initMsg))),
        row("salt", text(salt.toBase64())),
        row("funds", text(JSON.stringify(funds))),
      ]);
      if (admin) {
        components.push(row("admin", address(admin.toString())));
      }
    } else if ("execute" in msg) {
      const { contract, msg: execMsg, funds } = msg.execute;
      components = components.concat([
        row(`message #${idx + 1}`, text("execute")),
        row("contract", address(contract.toString())),
        row("msg", text(JSON.stringify(execMsg))),
        row("funds", text(JSON.stringify(funds))),
      ]);
    } else if ("migrate" in msg) {
      const { contract, newCodeHash, msg: migrMsg } = msg.migrate;
      components = components.concat([
        row(`message #${idx + 1}`, text("migrate")),
        row("contract", address(contract.toString())),
        row("new code hash", text(newCodeHash.toHex())),
        row("msg", text(JSON.stringify(migrMsg))),
      ]);
    }
  }
  return components;
}

/**
 * Sign a transaction. The private key is generated by entrophy provided by
 * MetaMask.
 * @param params Parameters necessary for signing the transaction, including the
 * messages, the chain ID, the signer address, and the signer's sequence number.
 * @returns The signature bytes.
 */
export async function signTransaction({
  sender,
  chainId,
  sequence,
  msgs,
}: SignTransactionParams): Promise<Uint8Array> {
  // display transaction details and let user confirm it
  //
  // API reference for the`snap_dialog` method:
  // https://docs.metamask.io/snaps/reference/snaps-api/#confirmation-dialog
  const confirmed = await snap.request({
    method: "snap_dialog",
    params: {
      type: "confirmation",
      content: panel([
        heading("Confirm transaction"),
        row("Sender", address(sender.toString())),
        row("Chain ID", text(chainId)),
        row("Sequence", text(sequence.toString())),
        ...displayMessages(msgs),
      ]),
    },
  });

  if (!confirmed) {
    throw new Error("User denied transaction");
  }

  // user has confirmed the transaction. now we request the entropy from
  // MetaMask, generate the private key, and sign the transaction.
  //
  // API reference for the `snap_getBip44Entropy` method:
  // https://docs.metamask.io/snaps/reference/snaps-api/#snap_getbip44entropy
  const hdNode = await snap.request({
    method: "snap_getBip44Entropy",
    params: {
      coinType: ETHEREUM_COIN_TYPE,
    },
  });

  const deriver = await getBIP44AddressKeyDeriver(hdNode);
  const key = await deriver(0);
  if (!key.privateKeyBytes) {
    throw new Error("failed to retrieve private key from MetaMask");
  }
  const signingKey = await SigningKey.fromPrivateKey(key.privateKeyBytes);

  // return the signed transaction to the webapp.
  // the webapp will handle transaction broadcasting.
  return await signingKey.signTx(msgs, sender, chainId, sequence);
}

/**
 * Logics for handling RPC requests.
 */
export const onRpcRequest: OnRpcRequestHandler = async ({ request }): Promise<Json> => {
  switch (request.method) {
    case "signTransaction": {
      const params = request.params as unknown as SignTransactionParams;
      const signature = await signTransaction(params);
      // MetaMask requires us to return a `Json` type, but our signature is an
      // Uint8Array. therefore we have to encode the signature into a type that
      // satisfies the `Json` interface. here I choose to encode it to a `string`
      // in base64. casting to a `number[]` may also be an option; not sure which
      // one is more efficient, but for users the difference is probably negligible.
      return encodeBase64(signature);
    }
    default: {
      throw new Error(`Unknown method: ${request.method}`);
    }
  }
};
