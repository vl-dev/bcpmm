import { getAdminKeypair } from "../admin-keypair";
import { useQuery } from "@tanstack/react-query";

export const useAdminKeypair = () => {
  return useQuery({
    queryKey: ['adminKeypair'],
    queryFn: async () => {
      return getAdminKeypair();
    },
  });
};