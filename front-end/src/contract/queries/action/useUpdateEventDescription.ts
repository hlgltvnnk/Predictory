import { useMutation, useQueryClient } from "@tanstack/react-query";
import { usePredictoryService } from "@/providers/PredictoryService";
import { useWallet } from "@solana/wallet-adapter-react";
import { useConnection } from "@solana/wallet-adapter-react";

const useUpdateEventDescription = () => {
  const { publicKey, sendTransaction } = useWallet();
  const { connection } = useConnection();
  const queryClient = useQueryClient();
  const { predictoryService } = usePredictoryService();

  return useMutation({
    mutationFn: async ({
      eventId,
      description,
    }: {
      eventId: string;
      description: string;
    }) => {
      if (!publicKey) throw new Error("PublicKey not found");
      if (!predictoryService) throw new Error("Predictory service not found");

      const authority = publicKey;
      const transaction = await predictoryService.action.updateEventDescription(
        authority,
        eventId,
        description,
      );
      const tx = await sendTransaction(transaction, connection);
      return tx;
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["allEvents"] });
      //TODO: or invalidate event
      // queryClient.invalidateQueries({ queryKey: ["event", eventId] });
    },
  });
};

export default useUpdateEventDescription;
